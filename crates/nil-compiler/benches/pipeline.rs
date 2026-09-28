use nil_compiler::{
    compile,
    evaluator::{Limits, execute},
};
use std::{hint::black_box, time::Instant};

fn main() {
    let source = include_str!("../../../examples/add.nil");
    let program = compile(source).unwrap();
    let entry = program.function(0).unwrap();
    for _ in 0..1000 {
        black_box(compile(black_box(source)).unwrap());
        assert_eq!(
            execute(&program.hir, entry, &[], Limits::default()).unwrap(),
            42
        );
    }
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(compile(black_box(source)).unwrap());
    }
    let frontend = start.elapsed().as_nanos();
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(execute(black_box(&program.hir), entry, &[], Limits::default()).unwrap());
    }
    let runtime = start.elapsed().as_nanos();
    println!(
        "{{\"schema\":1,\"fixture\":\"examples/add.nil\",\"iterations\":{iterations},\"source_bytes\":{},\"source_characters\":{},\"frontend_mean_ns\":{},\"interpreter_mean_ns\":{},\"source_tokens\":null,\"backend_ns\":null,\"binary_bytes\":null}}",
        source.len(),
        source.chars().count(),
        frontend / iterations,
        runtime / iterations
    );
}
