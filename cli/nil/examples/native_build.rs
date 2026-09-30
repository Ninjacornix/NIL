//! Build-stage measurements for native/interpreter/Python comparisons.
use nil_compiler::{SourceProfile, compile_with_profile};
use nil_llvm::{Instrumentation, Optimization, Options, build};
use std::{env, fs, path::Path, time::Instant};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    assert!(
        (5..=6).contains(&args.len()),
        "usage: native_build PROFILE SOURCE ENTRY OUTPUT O0|O2 [auto|bounded|unbounded]"
    );
    let instrumentation = match args.get(5).map(String::as_str).unwrap_or("auto") {
        "auto" => Instrumentation::ProfileDefault,
        "bounded" => Instrumentation::Bounded,
        "unbounded" => Instrumentation::Unbounded,
        _ => panic!("unknown instrumentation"),
    };
    let source = fs::read_to_string(&args[1]).unwrap();
    let profile = SourceProfile::parse(&args[0]).unwrap();
    let start = Instant::now();
    let program = compile_with_profile(&source, profile).unwrap();
    let frontend = start.elapsed().as_nanos();
    let entry = program.function(args[2].parse().unwrap()).unwrap();
    let optimization = match args[4].as_str() {
        "O0" => Optimization::O0,
        "O2" => Optimization::O2,
        _ => panic!("unknown optimization"),
    };
    let stats = build(
        &program.hir,
        Path::new(&args[3]),
        Options {
            instrumentation,
            entry,
            optimization,
            ..Default::default()
        },
    )
    .unwrap();
    println!(
        "{{\"frontend_ns\":{frontend},\"ir_lowering_ns\":{},\"llvm_codegen_ns\":{},\"runtime_compile_ns\":{},\"link_ns\":{},\"backend_total_ns\":{},\"binary_bytes\":{}}}",
        stats.ir_lowering_ns,
        stats.llvm_codegen_ns,
        stats.runtime_compile_ns,
        stats.link_ns,
        stats.total_ns,
        stats.binary_bytes
    );
}
