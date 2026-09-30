use nil_fuzz::{
    CORPUS, frontend,
    generate::{Case, Expr, Random},
    hir, mutate, valid,
};
#[test]
fn seeded_programs_mutations_and_hir_are_reproducible() {
    for seed in 0..1000 {
        let case = Case::new(seed);
        assert_eq!(case.source, Case::new(seed).source);
        assert_eq!(case.arguments, Case::new(seed).arguments);
        frontend(&case.source);
        valid(&case);
        let mut r = Random(seed);
        for _ in 0..4 {
            if let Ok(text) = std::str::from_utf8(&mutate(case.source.as_bytes(), &mut r)) {
                frontend(text);
            }
        }
        hir(&case, &mut r);
    }
}
fn count(e: &Expr, counts: &mut [usize; 17], loop_depth: usize) {
    match e {
        Expr::Int(_) => counts[0] += 1,
        Expr::Bool(_) => counts[1] += 1,
        Expr::Param(_) => counts[2] += 1,
        Expr::Binary(op, a, b) => {
            counts[3 + *op] += 1;
            count(a, counts, loop_depth);
            count(b, counts, loop_depth);
        }
        Expr::Compare(op, a, b) => {
            counts[7 + *op] += 1;
            count(a, counts, loop_depth);
            count(b, counts, loop_depth);
        }
        Expr::If(c, a, b) => {
            counts[13] += 1;
            for e in [c, a, b] {
                count(e, counts, loop_depth);
            }
        }
        Expr::Call(_, args) => {
            counts[14] += 1;
            for e in args {
                count(e, counts, loop_depth);
            }
        }
        Expr::Loop(init, c, updates, finish) => {
            counts[15] += 1;
            if loop_depth > 0 {
                counts[16] += 1;
            }
            for e in init {
                count(e, counts, loop_depth);
            }
            count(c, counts, loop_depth + 1);
            for e in updates {
                count(e, counts, loop_depth + 1);
            }
            count(finish, counts, loop_depth + 1);
        }
    }
}
#[test]
fn generation_exercises_every_current_operation_and_nested_loops() {
    let mut counts = [0; 17];
    for seed in 0..1000 {
        for tree in Case::new(seed).trees() {
            count(tree, &mut counts, 0);
        }
    }
    assert!(
        counts.iter().all(|n| *n > 0),
        "missing generator capability: {counts:?}"
    );
    assert!(counts[16] > 0, "no nested loop generated");
}
#[test]
fn malformed_unicode_depth_and_source_size_limits_are_resilient() {
    for source in CORPUS {
        frontend(source);
    }
    for source in [
        "=💥",
        "1=λ",
        "=\0",
        "4097=a",
        "1=aa",
        "=9223372036854775808",
        "=-9223372036854775809",
        "=1+true",
        "=b()",
        "1=a(-1)",
    ] {
        frontend(source);
    }
    for depth in [32, 127, 128, 129, 1024] {
        frontend(&format!("={}0{}", "(".repeat(depth), ")".repeat(depth)));
        frontend(&format!("={}true{}", "(".repeat(depth), ")".repeat(depth)));
    }
    let max = nil_compiler::parser::MAX_SOURCE_BYTES;
    frontend(&" ".repeat(max));
    frontend(&" ".repeat(max + 1));
}

#[test]
fn externally_constructed_hir_rejects_excessive_region_depth() {
    use nil_hir::*;
    let mut instruction = Instruction {
        operation: Operation::Constant(1),
        ty: Type::I64,
        span: None,
    };
    for _ in 0..MAX_REGION_DEPTH + 2 {
        instruction = Instruction {
            operation: Operation::If {
                condition: ValueId(0),
                then_region: Region {
                    instructions: vec![instruction],
                    results: vec![ValueId(2)],
                },
                else_region: Region {
                    instructions: vec![],
                    results: vec![ValueId(1)],
                },
            },
            ty: Type::I64,
            span: None,
        };
    }
    let program = Program {
        arithmetic: Arithmetic::Wrapping,
        functions: vec![Function {
            parameters: vec![Type::Bool, Type::I64],
            result_type: Type::I64,
            instructions: vec![instruction],
            result: ValueId(2),
            return_span: None,
        }],
    };
    let error = validate(program).unwrap_err();
    assert_eq!(error.code, "E008");
    assert_eq!(error.phase, Phase::Check);
    assert_eq!(error.message, "HIR region nesting limit exceeded");
}
