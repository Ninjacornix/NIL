//! Time the complete frontend, independently of execution.
use nil_compiler::{SourceProfile, compile_with_profile};
use std::{env, fs, hint::black_box, process::ExitCode, time::Instant};
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: paired_frontend PROFILE FILE ITERATIONS REPEATS".into());
    }
    let profile = SourceProfile::parse(&args[0]).ok_or("unknown profile")?;
    let source = fs::read_to_string(&args[1]).map_err(|e| e.to_string())?;
    let iterations = args[2].parse::<u64>().map_err(|e| e.to_string())?;
    let repeats = args[3].parse::<u64>().map_err(|e| e.to_string())?;
    if !(1..=100_000).contains(&iterations) || !(1..=100).contains(&repeats) {
        return Err("measurement bounds exceeded".into());
    }
    for _ in 0..100 {
        black_box(compile_with_profile(black_box(&source), profile).map_err(|e| e.to_string())?);
    }
    let mut samples = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(
                compile_with_profile(black_box(&source), profile).map_err(|e| e.to_string())?,
            );
        }
        samples.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    println!("{{\"ns_per_compile\":{samples:?}}}");
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
