use std::{env, process::ExitCode};

const HELP: &str = "NIL — Neural Instruction Language

Usage: nil [--help | --version]

Repository scaffold only. Parsing and execution are not implemented.
Development plan: docs/ROADMAP.md
First implementation tasks: docs/milestones/01-minimal-executable.md";

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => println!("{HELP}"),
        [arg] if arg == "--help" => println!("{HELP}"),
        [arg] if arg == "--version" => println!("nil {} (scaffold)", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!("error: unsupported arguments; use nil --help");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
