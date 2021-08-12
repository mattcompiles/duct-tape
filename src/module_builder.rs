use ast::*;
use std::fs;
use std::path::PathBuf;
use swc_common::comments::SingleThreadedComments;
use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecmascript::ast;
use swc_ecmascript::codegen::text_writer::JsWriter;

use crate::js_module::Dependency;
use crate::js_module::JsModule;
use crate::parser::parse;
use crate::transforms::runtime_imports::runtime_imports;
use crate::utils::create_module_id;

pub struct ModuleBuilder {
    project_root: PathBuf,
}

impl ModuleBuilder {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn build(&self, filepath: PathBuf) -> Result<(JsModule, Vec<Dependency>), String> {
        let id = create_module_id(&filepath, &self.project_root);
        let source_map = Lrc::new(SourceMap::default());
        let src_code = fs::read_to_string(&filepath).unwrap();
        let (module, comments) = match parse(&src_code, &filepath.to_str().unwrap(), &source_map) {
            Err(_) => return Err(String::from("Error parsing module")),
            Ok(module) => module,
        };

        let (final_ast, dependencies) = runtime_imports(module, &filepath, &self.project_root);

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

        Ok((JsModule { id, filepath, code }, dependencies))
    }
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
