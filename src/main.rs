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
        .get_matches();

    let project_root = env::current_dir().expect("Couldn't access CWD");
    let entrypoint = matches.value_of("entrypoint").expect("Missing entrpoint");
    let config = Config {
        entrypoint: project_root.join(entrypoint),
        project_root,
    };

    compile(config);
}
