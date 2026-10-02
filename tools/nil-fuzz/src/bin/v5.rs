use std::{path::PathBuf, process::ExitCode};
fn run() -> Result<(), String> {
    let mut seed = 5130572;
    let mut cases = 288;
    let mut out = PathBuf::from("fuzz/artifacts/expr-v5");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() % 2 != 0 {
        return Err("usage: v5 [--seed U64] [--cases N] [--out DIR]".into());
    }
    for pair in args.chunks_exact(2) {
        match pair[0].as_str() {
            "--seed" => seed = pair[1].parse().map_err(|_| "invalid seed")?,
            "--cases" => cases = pair[1].parse().map_err(|_| "invalid cases")?,
            "--out" => out = pair[1].clone().into(),
            _ => return Err(format!("unknown flag {}", pair[0])),
        }
    }
    if cases == 0 || cases > 100000 {
        return Err("require 1..100000 cases".into());
    }
    nil_fuzz::application::campaign(seed, cases, &out)
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
