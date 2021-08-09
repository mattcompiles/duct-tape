use std::env;
use std::path::PathBuf;
use std::process;

mod js_module;
mod module_graph;
mod parser;

fn main() {
    let entry = {
        let mut path = PathBuf::from(env::current_dir().expect("Couldn't access CWD"));
        path.push("fixture/index.js");
        path
    };

    let mut graph = module_graph::ModuleGraph::new();

    let entry_module = match js_module::JsModule::new(&entry) {
        Ok(module) => module,
        Err(err) => {
            println!("{}", err);
            process::exit(1)
        }
    };

    graph.load_module(entry_module);

    println!("{:?}", graph);
}
