use std::{path::PathBuf, process::ExitCode};
fn run() -> Result<(), String> {
    let mut seed = 5130572;
    let mut cases = nil_fuzz::application::FAMILY_COUNT;
    let mut scope = "all".to_owned();
    let mut out = PathBuf::from("fuzz/artifacts/expr-v5");
    // Round 27 P1. Cases are a pure fold, so --jobs only changes scheduling.
    let mut jobs = 1usize;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() % 2 != 0 {
        return Err("usage: v5 [--seed U64] [--cases N] [--out DIR] [--scope S] [--jobs N]".into());
    }
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--seed" => seed = pair[1].parse().map_err(|_| "invalid seed")?,
            "--cases" => cases = pair[1].parse().map_err(|_| "invalid cases")?,
            "--out" => out = pair[1].clone().into(),
            "--scope" => scope = pair[1].clone(),
            "--jobs" => jobs = pair[1].parse().map_err(|_| "invalid jobs")?,
            _ => return Err(format!("unknown flag {}", pair[0])),
        }
    }
    if jobs == 0 || jobs > 64 {
        return Err("require 1..64 jobs".into());
    }
    if cases == 0 || cases > 100000 {
        return Err("require 1..100000 cases".into());
    }
    match scope.as_str() {
        "all" => nil_fuzz::application::campaign(seed, cases, &out, jobs),
        "chains" => nil_fuzz::application::chain_campaign(seed, &out, jobs),
        "stacks" => nil_fuzz::application::stack_campaign(seed, &out, jobs),
        "io-faults" => nil_fuzz::application::io_fault_campaign(seed, &out, jobs),
        _ => Err("require --scope all|io-faults|stacks|chains".into()),
    }
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
