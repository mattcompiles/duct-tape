mod diagnostics;
mod js_module;
mod module_graph;
mod module_loader;
mod parser;
mod template;
mod transforms;
mod utils;

use crate::diagnostics::Diagnostics;
use crate::module_graph::ModuleGraph;
use crate::utils::create_module_id;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

pub struct Config {
    pub project_root: PathBuf,
    pub entrypoint: PathBuf,
    pub output_dir: PathBuf,
}

pub struct Compilation {
    pub config: Config,
    pub diagnostics: Diagnostics,
    pub graph: ModuleGraph,
}

pub fn compile(config: Config) {
    println!(
        "Entrypoint: '{}'\nOutput dir: '{}'\nProject root: '{}'",
        config.entrypoint.to_str().unwrap(),
        config.output_dir.to_str().unwrap(),
        config.project_root.to_str().unwrap()
    );
    let start_time = Instant::now();
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

    let chunk = template::render_chunk(&c.graph.entrypoints[0], &c);
    let output_filepath = c.config.output_dir.join("main.js");
    emit_file(&output_filepath.to_str().unwrap(), &chunk).expect("Failed to write chunk");
    let elapsed_time = start_time.elapsed();
    c.diagnostics.print();
    println!("Build complete in {}ms", elapsed_time.as_millis());
}

fn emit_file(file_path: &str, contents: &str) -> std::io::Result<()> {
    let mut file = File::create(file_path)?;
    file.write_all(contents.as_bytes())?;
    Ok(())
}
