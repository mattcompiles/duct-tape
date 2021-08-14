mod compiler;
mod diagnostics;
mod js_module;
mod module_graph;
mod module_loader;
mod parser;
mod template;
mod transforms;
mod utils;

use compiler::Config;
use std::env;
use std::path::PathBuf;

fn main() {
    let project_root = env::current_dir().expect("Couldn't access CWD");
    let entrypoint = {
        let mut path = PathBuf::from(&project_root);
        path.push("fixture/index.js");
        path
    };

    let config = Config {
        entrypoint,
        project_root,
    };

    compiler::compile(config);
}
