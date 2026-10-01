//! Repeated frontend/checker timings with file I/O and HIR cloning excluded.
use nil_compiler::{SourceProfile, compile_with_profile};
use std::{env, fs, hint::black_box, time::Instant};
fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(args.len(), 1, "usage: typecheck_bench SOURCE");
    let source = fs::read_to_string(&args[0]).unwrap();
    let program = compile_with_profile(&source, SourceProfile::ExprV4).unwrap();
    let mut frontend = vec![];
    let mut validation = vec![];
    for _ in 0..101 {
        let start = Instant::now();
        let result = black_box(compile_with_profile(
            black_box(&source),
            SourceProfile::ExprV4,
        ))
        .unwrap();
        frontend.push(start.elapsed().as_nanos());
        drop(result);
        let raw = program.hir.program().clone();
        let start = Instant::now();
        let result = black_box(nil_compiler::hir::validate(black_box(raw))).unwrap();
        validation.push(start.elapsed().as_nanos());
        drop(result);
    }
    frontend.sort_unstable();
    validation.sort_unstable();
    let entry = &program.hir.program().functions[0];
    let input_slots: usize = entry.parameters.iter().map(|ty| ty.slots()).sum();
    println!(
        "{{\"samples\":101,\"frontend_median_ns\":{},\"hir_validation_median_ns\":{},\"input_slots\":{input_slots},\"input_payload_bytes\":{},\"result_slots\":{}}}",
        frontend[50],
        validation[50],
        input_slots * 8,
        entry.result_type.slots()
    );
}
