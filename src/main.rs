use clap::{App, Arg};
use ducttape::{compile, Config};
use std::env;

fn main() {
    let matches = App::new("duct-tape")
        .arg(
            Arg::with_name("entrypoint")
                .help("Sets the entrypoint to bundle")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("output_dir")
                .help("Sets the output directory")
                .default_value("dist")
                .value_name("output-dir"),
        )
        .get_matches();

    let project_root = env::current_dir().expect("Couldn't access CWD");
    let entrypoint = matches.value_of("entrypoint").expect("Missing entrpoint");
    let output_dir = matches.value_of("output_dir").expect("Missing output-dir");

    let config = Config {
        entrypoint: project_root.join(entrypoint),
        output_dir: project_root.join(output_dir),
        project_root,
    };

    compile(config);
}
