//! Measure already-compiled NIL interpreter calls for the paired corpus.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, execute},
};
use std::{env, fs, hint::black_box, process::ExitCode, time::Instant};

fn run() -> Result<(), String> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    let profile = if args.first().is_some_and(|arg| arg == "--profile") {
        if args.len() < 2 {
            return Err("missing profile name".into());
        }
        let profile = SourceProfile::parse(&args[1]).ok_or("unknown source profile")?;
        args.drain(..2);
        profile
    } else {
        SourceProfile::LinesV0
    };
    if args.len() < 6 {
        return Err("usage: paired_runtime [--profile lines-v0|expr-v0] FILE FUNCTION EXPECTED WARMUP ITERATIONS REPEATS [I64_ARGUMENT...]".into());
    }
    let parse = |index: usize| -> Result<u64, String> {
        args[index]
            .parse::<u64>()
            .map_err(|_| format!("invalid unsigned integer: {}", args[index]))
    };
    let label = u32::try_from(parse(1)?).map_err(|_| "function label exceeds u32".to_owned())?;
    let expected = args[2]
        .parse::<i64>()
        .map_err(|_| "invalid expected value".to_owned())?;
    let warmup = parse(3)?;
    let iterations = parse(4)?;
    let repeats = parse(5)?;
    if iterations == 0 || repeats == 0 || iterations > 10_000_000 || repeats > 100 {
        return Err("iterations must be 1..=10000000 and repeats 1..=100".into());
    }
    let values: Vec<i64> = args[6..]
        .iter()
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| format!("invalid i64: {value}"))
        })
        .collect::<Result<_, _>>()?;
    let source = fs::read_to_string(&args[0]).map_err(|error| error.to_string())?;
    let program = compile_with_profile(&source, profile).map_err(|error| error.to_string())?;
    let entry = program
        .function(label)
        .ok_or_else(|| format!("unknown function {label}"))?;
    let invoke = || {
        execute(&program.hir, entry, black_box(&values), Limits::default())
            .map_err(|error| error.to_string())
    };
    for _ in 0..warmup {
        if black_box(invoke()?) != expected {
            return Err("warmup result differs from expected".into());
        }
    }
    let mut samples = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        for _ in 0..iterations {
            if black_box(invoke()?) != expected {
                return Err("timed result differs from expected".into());
            }
        }
        samples.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    print!("{{\"ns_per_call\":[");
    for (index, sample) in samples.iter().enumerate() {
        if index != 0 {
            print!(",");
        }
        print!("{sample}");
    }
    println!("]}}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
