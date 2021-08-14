use crate::diagnostics::Diagnostics;
use crate::module_graph::ModuleGraph;
use crate::module_loader;
use crate::template;
use crate::utils::create_module_id;
use std::path::PathBuf;

pub struct Config {
    pub project_root: PathBuf,
    pub entrypoint: PathBuf,
}

pub struct Compilation {
    pub config: Config,
    pub diagnostics: Diagnostics,
    pub graph: ModuleGraph,
}

pub fn compile(config: Config) {
    let mut c = Compilation {
        diagnostics: Diagnostics::new(),
        graph: ModuleGraph::new(),
        config,
    };

    c.graph.add_entrypoint(create_module_id(
        &c.config.entrypoint,
        &c.config.project_root,
    ));

    module_loader::load_entrypoint(&mut c);

    println!("{}", template::render_chunk(&c.graph.entrypoints[0], &c));

    c.diagnostics.print();
}
