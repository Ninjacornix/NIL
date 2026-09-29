use nil_compiler::{
    SourceProfile, compile_with_profile, dump,
    evaluator::{Limits, execute},
    parser::MAX_SOURCE_BYTES,
};
use std::{env, fs::File, io::Read, process::ExitCode};

const HELP: &str = "NIL — Neural Instruction Language

Usage:
  nil --help
  nil --version
  nil [--profile lines-v0|expr-v0] check FILE
  nil [--profile lines-v0|expr-v0] hir FILE
  nil [--profile lines-v0|expr-v0] run FILE [FUNCTION_ID [I64_ARGUMENT...]]

Run defaults to function 0 and the lines-v0 profile. M1 uses the reference interpreter (no native codegen).
Development plan: docs/ROADMAP.md";

fn run(args: &[std::ffi::OsString]) -> Result<(), (u8, String)> {
    if args.is_empty() || (args.len() == 1 && args[0] == "--help") {
        println!("{HELP}");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--version" {
        println!(
            "nil {} (lines-v0, expr-v0, interpreter)",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    let usage = || (2, "E010 invalid arguments; use nil --help".to_owned());
    let (profile, args) = if args.first().is_some_and(|arg| arg == "--profile") {
        let name = args.get(1).and_then(|arg| arg.to_str()).ok_or_else(usage)?;
        let profile = SourceProfile::parse(name).ok_or_else(usage)?;
        (profile, &args[2..])
    } else {
        (SourceProfile::LinesV0, args)
    };
    if args.is_empty() {
        return Err(usage());
    }
    let command = args[0].to_str().ok_or_else(usage)?;
    if args.len() < 2
        || !matches!(command, "check" | "hir" | "run")
        || (command != "run" && args.len() != 2)
    {
        return Err(usage());
    }
    let mut label = 0;
    let mut values = Vec::new();
    if args.len() > 2 {
        label = args[2]
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(usage)?;
        for arg in &args[3..] {
            values.push(
                arg.to_str()
                    .and_then(|s| s.parse::<i64>().ok())
                    .ok_or_else(usage)?,
            );
        }
    }
    let mut source = String::new();
    File::open(&args[1])
        .and_then(|f| {
            f.take(MAX_SOURCE_BYTES as u64 + 1)
                .read_to_string(&mut source)
        })
        .map_err(|e| (1, format!("E010 {}: {e}", args[1].to_string_lossy())))?;
    let program = compile_with_profile(&source, profile).map_err(|e| (1, e.to_string()))?;
    match command {
        "check" => println!("ok"),
        "hir" => print!("{}", dump(&program.hir)),
        "run" => {
            let entry = program
                .function(label)
                .ok_or_else(|| (1, format!("E004 unknown entry function {label}")))?;
            let value = execute(&program.hir, entry, &values, Limits::default())
                .map_err(|e| (1, e.to_string()))?;
            println!("{value}");
        }
        _ => unreachable!(),
    }
    Ok(())
}
fn main() -> ExitCode {
    match run(&env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
