use crate::compiler::Compilation;
use crate::diagnostics::{Diagnostic, ModuleBuildSuccess};
use crate::js_module::{Dependency, JsModule};
use crate::parser::parse;
use crate::transforms::runtime_imports::runtime_imports;
use crate::utils::create_module_id;

use ast::*;
use crossbeam_channel::unbounded;
use rayon::ThreadPoolBuilder;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecmascript::ast;
use swc_ecmascript::codegen::text_writer::JsWriter;

enum WorkMsg<T> {
    Work(T),
    Exit,
}

enum ResultMsg<T> {
    Result(T),
    Exited,
}

pub fn load_entrypoint(c: &mut Compilation) {
    let (work_sender, work_receiver) = unbounded();
    let (result_sender, result_receiver) = unbounded();
    let pool = ThreadPoolBuilder::new()
        .build()
        .expect("Failed to create ThreadPool");
    let project_root = Arc::new(c.config.project_root.clone());

    thread::spawn(move || loop {
        match work_receiver.recv() {
            Ok(WorkMsg::Work(filepath)) => {
                let result_sender = result_sender.clone();
                let project_root = Arc::clone(&project_root);

                pool.install(move || {
                    let start = Instant::now();

                    let (module, dependencies) =
                        build_module(filepath, project_root).expect("Failed to build module");

                    result_sender
                        .send(ResultMsg::Result((module, dependencies, start.elapsed())))
                        .unwrap();
                });
            }
            Ok(WorkMsg::Exit) => {
                result_sender.send(ResultMsg::Exited).unwrap();
                break;
            }
            _ => panic!("Error receiving a WorkMsg."),
        }
    });

    work_sender
        .send(WorkMsg::Work(c.config.entrypoint.clone()))
        .unwrap();

    let mut module_build_count = 1;

    loop {
        match result_receiver.recv() {
            Ok(ResultMsg::Result((module, dependencies, duration))) => {
                c.diagnostics
                    .add_diagnostic(Diagnostic::ModuleBuildSuccess(ModuleBuildSuccess {
                        module_id: module.id.clone(),
                        duration,
                    }));
                module_build_count -= 1;
                for dep in dependencies {
                    if !c.graph.has_module(&module.id) {
                        module_build_count += 1;
                        work_sender
                            .send(WorkMsg::Work(dep.filepath.clone()))
                            .unwrap();
                    }
                    c.graph.add_dependency(&module.id, &dep.id);
                }
                println!(
                    "{} built, deps in queue: {}",
                    &module.id, module_build_count
                );
                c.graph.add_module(module);
                if module_build_count == 0 {
                    work_sender.send(WorkMsg::Exit).unwrap();
                }
            }
            Ok(ResultMsg::Exited) => {
                break;
            }
            _ => panic!("Error receiving a ResultMsg."),
        }
    }
}

fn build_module(
    filepath: PathBuf,
    project_root: Arc<PathBuf>,
) -> Result<(JsModule, Vec<Dependency>), String> {
    println!("Building {}", filepath.to_str().unwrap());
    let id = create_module_id(&filepath, &project_root);
    let source_map = Lrc::new(SourceMap::default());
    let src_code = fs::read_to_string(&filepath).unwrap();
    let (module, comments) = match parse(&src_code, &filepath.to_str().unwrap(), &source_map) {
        Err(_) => return Err(String::from("Error parsing module")),
        Ok(module) => module,
    };

    let (final_ast, dependencies, module_type) = runtime_imports(module, &filepath, &project_root);

    let buf = match emit(&final_ast, source_map, comments) {
        Err(_) => return Err(format!("Failed to emit buffer: {}", &id)),
        Ok(value) => value,
    };

    let code = match String::from_utf8(buf) {
        Err(_) => {
            return Err(format!(
                "Failed to convert UTF-8 buffer to string buffer: {}",
                &id,
            ))
        }
        Ok(value) => value,
    };

    Ok((
        JsModule {
            id,
            filepath,
            code,
            module_type,
        },
        dependencies,
    ))
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
