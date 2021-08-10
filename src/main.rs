use std::env;
use std::path::PathBuf;
use std::process;

use crate::js_module::JsModule;

mod js_module;
mod module_graph;
mod parser;
mod template;

fn main() {
    let entry = {
        let mut path = PathBuf::from(env::current_dir().expect("Couldn't access CWD"));
        path.push("fixture/index.js");
        path
    };

    let mut graph = module_graph::ModuleGraph::new();

    let entry_module = match JsModule::new(&entry) {
        Ok(module) => module,
        Err(err) => {
            println!("{}", err);
            process::exit(1)
        }
    };

    graph.load_module(entry_module.clone());

    println!(
        "{}",
        template::render_chunk(&graph, entry_module.filename.to_str().unwrap())
    );
}
