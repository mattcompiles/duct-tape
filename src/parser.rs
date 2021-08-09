use std::path::Path;

use swc_common::comments::SingleThreadedComments;
use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecmascript::ast::Module;
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::{PResult, Parser, StringInput, Syntax, TsConfig};

pub fn parse(
    code: &str,
    filename: &str,
    source_map: &Lrc<SourceMap>,
) -> PResult<(Module, SingleThreadedComments)> {
    let source_file = source_map.new_source_file(
        FileName::Real(Path::new(filename).to_path_buf()),
        code.into(),
    );
    let comments = SingleThreadedComments::default();
    let syntax = {
        let mut tsconfig = TsConfig::default();
        tsconfig.tsx = true;
        tsconfig.dynamic_import = true;
        Syntax::Typescript(tsconfig)
    };
    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::from(&*source_file),
        Some(&comments),
    );
    let mut parser = Parser::new_from(lexer);
    match parser.parse_module() {
        Err(err) => Err(err),
        Ok(module) => Ok((module, comments)),
    }
}
