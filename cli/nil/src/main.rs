use nil_compiler::{SourceProfile, compile_with_profile, dump, parser::MAX_SOURCE_BYTES};
use std::{env, fs::File, io::Read, process::ExitCode};

const HELP: &str = "NIL — Neural Instruction Language

Usage:
  nil --help
  nil --version
  nil [--profile lines-v0|expr-v0|expr-v1|expr-v2|expr-v3|expr-v4|expr-v5] check FILE
  nil [--profile PROFILE] [--bounded|--unbounded] llvm FILE
  nil [--profile PROFILE] [--bounded|--unbounded] build FILE -o OUTPUT [--entry ID] [-O0|-O2]
  nil [--profile lines-v0|expr-v0|expr-v1|expr-v2|expr-v3|expr-v4|expr-v5] hir FILE
  nil [--profile lines-v0|expr-v0|expr-v1|expr-v2|expr-v3|expr-v4|expr-v5] [--bounded|--unbounded] run FILE [FUNCTION_ID [ARGUMENT...]]

Run defaults to function 0 and the expr-v0 profile. Use --profile lines-v0 for the legacy line syntax. run compiles and executes host-native LLVM code; build saves an executable. Clang 15+ is required. expr-v3/v4 use wrapping i64 and no resource counting by default. expr-v4 adds typed bool/array functions; array parameters consume flattened i64 slots (bool 0/1).
expr-v5 adds dynamic buffers, byte/text values and explicit file operations; s arguments are text and v arguments use [1,2,3].
Development plan: docs/ROADMAP.md";

fn run(args: &[std::ffi::OsString]) -> Result<(), (u8, String)> {
    if args.is_empty() || (args.len() == 1 && args[0] == "--help") {
        println!("{HELP}");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--version" {
        println!(
            "nil {} (expr-v0 default, lines-v0 optional, LLVM native default)",
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
        (SourceProfile::default(), args)
    };
    let (instrumentation, args) = match args.first().and_then(|s| s.to_str()) {
        Some("--bounded") => (nil_llvm::Instrumentation::Bounded, &args[1..]),
        Some("--unbounded") => (nil_llvm::Instrumentation::Unbounded, &args[1..]),
        _ => (nil_llvm::Instrumentation::ProfileDefault, args),
    };
    if args.is_empty() {
        return Err(usage());
    }
    let command = args[0].to_str().ok_or_else(usage)?;
    if args.len() < 2
        || !matches!(command, "check" | "hir" | "run" | "llvm" | "build")
        || (!matches!(command, "run" | "build") && args.len() != 2)
    {
        return Err(usage());
    }
    if instrumentation != nil_llvm::Instrumentation::ProfileDefault
        && !matches!(command, "run" | "build" | "llvm")
    {
        return Err(usage());
    }
    let mut label = 0;
    let mut values = Vec::new();
    if command == "run" && args.len() > 2 {
        label = args[2]
            .to_str()
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(usage)?;
        for arg in &args[3..] {
            let text = arg.to_str().ok_or_else(usage)?;
            if profile != SourceProfile::ExprV5 {
                text.parse::<i64>().map_err(|_| usage())?;
            }
            values.push(text.to_owned());
        }
    }
    let build_request = if command == "build" {
        let mut output = None;
        let mut optimization = nil_llvm::Optimization::O2;
        let mut selected_optimization = false;
        let mut selected_entry = false;
        let mut index = 2;
        while index < args.len() {
            match args[index].to_str() {
                Some("-o") if output.is_none() => {
                    index += 1;
                    output = Some(std::path::PathBuf::from(args.get(index).ok_or_else(usage)?));
                }
                Some("--entry") if !selected_entry => {
                    index += 1;
                    label = args
                        .get(index)
                        .and_then(|s| s.to_str())
                        .and_then(|s| s.parse::<u32>().ok())
                        .ok_or_else(usage)?;
                    selected_entry = true;
                }
                Some("-O0" | "-O2") if !selected_optimization => {
                    optimization = if args[index] == "-O0" {
                        nil_llvm::Optimization::O0
                    } else {
                        nil_llvm::Optimization::O2
                    };
                    selected_optimization = true;
                }
                _ => return Err(usage()),
            }
            index += 1;
        }
        Some((output.ok_or_else(usage)?, optimization))
    } else {
        None
    };
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
        "llvm" => print!(
            "{}",
            nil_llvm::emit_llvm_with_instrumentation(&program.hir, instrumentation)
        ),
        "build" => {
            let (output, optimization) = build_request.unwrap();
            if output.canonicalize().ok().is_some_and(|path| {
                Some(path) == std::path::Path::new(&args[1]).canonicalize().ok()
            }) {
                return Err((1, "E010 output must differ from source".into()));
            }
            let entry = program
                .function(label)
                .ok_or_else(|| (1, format!("E004 unknown entry function {label}")))?;
            nil_llvm::build(
                &program.hir,
                &output,
                nil_llvm::Options {
                    instrumentation,
                    entry,
                    optimization,
                    ..Default::default()
                },
            )
            .map_err(|e| (1, e.to_string()))?;
            println!("built {}", output.display());
        }
        "run" => {
            let entry = program
                .function(label)
                .ok_or_else(|| (1, format!("E004 unknown entry function {label}")))?;
            let output = nil_llvm::run_arguments(
                &program.hir,
                nil_llvm::Options {
                    instrumentation,
                    entry,
                    ..Default::default()
                },
                &values,
            )
            .map_err(|e| (1, e.to_string()))?;
            if !output.status.success() {
                return Err((
                    output
                        .status
                        .code()
                        .and_then(|c| u8::try_from(c).ok())
                        .unwrap_or(1),
                    if output.stderr.is_empty() {
                        format!("E011 native process failed: {}", output.status)
                    } else {
                        String::from_utf8_lossy(&output.stderr)
                            .trim_end()
                            .to_owned()
                    },
                ));
            }
            use std::io::Write;
            std::io::stdout()
                .write_all(&output.stdout)
                .map_err(|e| (1, format!("E010 cannot write result: {e}")))?;
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
