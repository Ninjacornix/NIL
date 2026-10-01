//! Bounded scalar oracle interface. Source is parsed as NIL, never host code.
use nil_compiler::{SourceProfile, compile_with_profile, evaluator, hir};
use std::{env, fs, time::Instant};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let profile = SourceProfile::parse(&args[0]).expect("profile");
    let source = fs::read_to_string(&args[1]).expect("UTF-8 source");
    let start = Instant::now();
    let program = match compile_with_profile(&source, profile) {
        Ok(p) => p,
        Err(d) => {
            println!("ERR\t{:?}\t{}\t{}", d.phase, start.elapsed().as_nanos(), d);
            return;
        }
    };
    println!("OK\t{}", start.elapsed().as_nanos());
    for line in fs::read_to_string(&args[2]).expect("vectors").lines() {
        let values = line
            .split_whitespace()
            .map(|v| v.parse::<i64>().expect("i64"))
            .collect::<Vec<_>>();
        let start = Instant::now();
        match evaluator::execute(
            &program.hir,
            hir::FunctionId(0),
            &values,
            evaluator::Limits {
                steps: 100_000,
                call_depth: 256,
            },
        ) {
            Ok(v) => println!("VALUE\t{}\t{v}", start.elapsed().as_nanos()),
            Err(d) => println!("ERR\tExecute\t{}\t{}", start.elapsed().as_nanos(), d),
        }
    }
}
