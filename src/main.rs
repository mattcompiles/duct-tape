mod js_module;
mod module_builder;
mod module_graph;
mod parser;
mod template;
mod transforms;
mod utils;

use crate::module_builder::ModuleBuilder;
use crate::module_graph::ModuleGraph;
use crate::utils::create_module_id;
use std::env;
use std::path::PathBuf;

fn main() {
    let project_root = env::current_dir().expect("Couldn't access CWD");
    let entrypoint = {
        let mut path = PathBuf::from(&project_root);
        path.push("fixture/index.js");
        path
    };

    let mut graph = ModuleGraph::new();

    graph.add_entrypoint(create_module_id(&entrypoint, &project_root));

    let builder = ModuleBuilder::new(project_root);
    let mut build_queue: Vec<PathBuf> = vec![entrypoint];

    while let Some(filepath) = build_queue.pop() {
        let (module, dependencies) = builder.build(filepath).expect("Failed to build module");
        for dep in dependencies {
            if !graph.has_module(&dep.id) {
                build_queue.push(dep.filepath);
            }
            graph.add_dependency(&module.id, &dep.id);
        }
        graph.add_module(module);
    }

    println!("{}", template::render_chunk(&graph, &graph.entrypoints[0]));
}
