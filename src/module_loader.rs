use crate::diagnostics::{Diagnostic, ModuleBuildSuccess};
use crate::js_module::ModuleType;
use crate::js_module::{Dependency, JsModule};
use crate::parser::parse;
use crate::transforms::runtime_imports::runtime_imports;
use crate::utils::create_module_id;
use crate::Compilation;
use node_resolve::Resolver;
use std::collections::HashSet;
use std::time::Duration;
use swc_atoms::JsWord;

use ast::*;
use crossbeam_channel::unbounded;
use rayon::ThreadPoolBuilder;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;
use swc_common::chain;
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecmascript::ast;
use swc_ecmascript::codegen::text_writer::JsWriter;
use swc_ecmascript::transforms::{react, typescript};
use swc_ecmascript::visit::FoldWith;

struct BuildModuleSuccess {
    filepath: PathBuf,
    code: String,
    module_type: ModuleType,
    dependencies: Vec<Dependency>,
    duration: Duration,
}

struct ResolveModule {
    source_filepath: PathBuf,
    request: JsWord,
    parent_module_id: String,
}

struct ResolveModuleSuccess {
    filepath: PathBuf,
    dep_id: String,
    parent_module_id: String,
    request: JsWord,
}

enum WorkMsg {
    ResolveModule(ResolveModule),
    BuildModule(PathBuf),
    Exit,
}

enum ResultMsg {
    BuildModule(BuildModuleSuccess),
    ResolveModule(ResolveModuleSuccess),
    Exited,
}

pub fn load_entrypoint(c: &mut Compilation) {
    let (work_sender, work_receiver) = unbounded();
    let (result_sender, result_receiver) = unbounded();
    let pool = ThreadPoolBuilder::new()
        .build()
        .expect("Failed to create ThreadPool");
    let project_root = c.config.project_root.clone();

    thread::spawn(move || loop {
        match work_receiver.recv() {
            Ok(WorkMsg::BuildModule(filepath)) => {
                let result_sender = result_sender.clone();

                pool.install(move || {
                    let result = build_module(filepath).expect("Failed to build module");

                    result_sender
                        .send(ResultMsg::BuildModule(result))
                        .expect("Failed to send BuildModule result from thread");
                });
            }
            Ok(WorkMsg::ResolveModule(work)) => {
                let result_sender = result_sender.clone();
                let project_root = project_root.clone();

                pool.install(move || {
                    let resolved_filepath = resolve_module(work.source_filepath, &work.request[..]);
                    let dep_id = create_module_id(&resolved_filepath, &project_root);

                    result_sender
                        .send(ResultMsg::ResolveModule(ResolveModuleSuccess {
                            filepath: resolved_filepath,
                            dep_id,
                            parent_module_id: work.parent_module_id,
                            request: work.request,
                        }))
                        .expect("Failed to send ResolveModule result from thread");
                });
            }
            Ok(WorkMsg::Exit) => {
                result_sender.send(ResultMsg::Exited).unwrap();
                break;
            }
            _ => panic!("Error receiving a WorkMsg."),
        }
    });

    // Trigger initial build by add entrypoint to work queue
    work_sender
        .send(WorkMsg::BuildModule(c.config.entrypoint.clone()))
        .unwrap();

    let mut active_work_count = 1;
    let mut found_modules: HashSet<String> = HashSet::new();

    loop {
        match result_receiver.recv() {
            Ok(ResultMsg::BuildModule(result)) => {
                let module_id = create_module_id(&result.filepath, &c.config.project_root.clone());
                c.diagnostics
                    .add_diagnostic(Diagnostic::ModuleBuildSuccess(ModuleBuildSuccess {
                        module_id: module_id.clone(),
                        duration: result.duration,
                    }));

                active_work_count -= 1;

                for dep in result.dependencies {
                    active_work_count += 1;
                    work_sender
                        .send(WorkMsg::ResolveModule(ResolveModule {
                            request: dep.request.clone(),
                            parent_module_id: module_id.clone(),
                            source_filepath: result.filepath.clone(),
                        }))
                        .expect("Failed to send ResolveModule reqest");
                }

                c.graph.add_module(JsModule {
                    id: module_id,
                    filepath: result.filepath,
                    code: result.code,
                    module_type: result.module_type,
                });

                if active_work_count == 0 {
                    work_sender.send(WorkMsg::Exit).unwrap();
                }
            }
            Ok(ResultMsg::ResolveModule(result)) => {
                let graph = &mut c.graph;

                graph
                    .get_module(&result.parent_module_id)
                    .expect("Failed to get requesting module")
                    .update_dep_src(&result.request, &result.dep_id);

                graph.add_dependency(&result.parent_module_id, &result.dep_id);

                if !found_modules.contains(&result.dep_id) {
                    found_modules.insert(result.dep_id.clone());
                    work_sender
                        .send(WorkMsg::BuildModule(result.filepath))
                        .expect("Failed to send BuildModule request");
                } else {
                    active_work_count -= 1;
                }
            }
            Ok(ResultMsg::Exited) => {
                break;
            }
            _ => panic!("Error receiving a ResultMsg."),
        }
    }
}

fn build_module(filepath: PathBuf) -> Result<BuildModuleSuccess, String> {
    let start = Instant::now();
    let source_map = Lrc::new(SourceMap::default());

    let src_code = fs::read_to_string(&filepath).expect(&format!(
        "Failed to read file: {}",
        &filepath.to_str().unwrap()
    ));
    let (module, comments) = match parse(&src_code, &filepath.to_str().unwrap(), &source_map) {
        Err(_) => return Err(String::from("Error parsing module")),
        Ok(module) => module,
    };

    let (module, dependencies, module_type) = runtime_imports(module);

    let final_ast = {
        let react_transform = react::react(
            source_map.clone(),
            Some(&comments),
            react::Options::default(),
        );
        let mut passes = chain!(typescript::strip(), react_transform);
        module.fold_with(&mut passes)
    };

    let buf = match emit(&final_ast, source_map, comments) {
        Err(_) => {
            return Err(format!(
                "Failed to emit buffer: {}",
                &filepath.to_str().unwrap()
            ))
        }
        Ok(value) => value,
    };

    let code = match String::from_utf8(buf) {
        Err(_) => {
            return Err(format!(
                "Failed to convert UTF-8 buffer to string buffer: {}",
                &filepath.to_str().unwrap(),
            ))
        }
        Ok(value) => value,
    };

    Ok(BuildModuleSuccess {
        filepath,
        code,
        module_type,
        dependencies,
        duration: start.elapsed(),
    })
}

fn emit(
    ast: &Module,
    source_map: Lrc<SourceMap>,
    comments: SingleThreadedComments,
) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = vec![];
    {
        let writer = Box::new(JsWriter::new(source_map.clone(), "\n", &mut buf, None));
        let config = swc_ecmascript::codegen::Config { minify: false };
        let mut emitter = swc_ecmascript::codegen::Emitter {
            cfg: config,
            comments: Some(&comments),
            cm: source_map.clone(),
            wr: writer,
        };
        emitter.emit_module(ast)?;
    }
    return Ok(buf);
}

fn resolve_module(source_filepath: PathBuf, request: &str) -> PathBuf {
    Resolver::new()
        .with_extensions(vec!["ts", "js", "mjs", "json"])
        .with_basedir(PathBuf::from(&source_filepath.parent().unwrap()))
        .resolve(request)
        .expect(&format!(
            "Failed to resolve {} from {:?}",
            request, &source_filepath
        ))
}
