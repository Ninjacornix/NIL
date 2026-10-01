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

#[test]
fn typed_source_mutations_preserve_deterministic_frontend_and_diagnostics() {
    use nil_compiler::SourceProfile;
    let corpus = [
        "(8)=@(a,0,0;b<#a;a,b+1,c+a[b];c)",
        "(8):8=@(a,[0;8],0;c<#a;a,b[c:a[#a-c-1]],c+1;b)",
        "(b):2=a?[1,2]:[3,4]",
        ":0=[]",
        "(3,i):3=a[b:7]",
        "(3):3=b(a)\n(3):3=a[1:a[0]+a[2]]",
    ];
    for seed in 0..1000 {
        let mut r = Random(seed);
        for source in corpus {
            if let Ok(text) =
                std::str::from_utf8(&nil_fuzz::mutate_typed(source.as_bytes(), &mut r))
            {
                nil_fuzz::frontend_with_profile(text, SourceProfile::ExprV4);
            }
        }
    }
    for depth in [127, 128, 129, 1024] {
        nil_fuzz::frontend_with_profile(
            &format!("={}[]", "#".repeat(depth)),
            SourceProfile::ExprV4,
        );
    }
}
#[test]
fn typed_hir_mutations_cannot_forge_array_lengths_or_operand_types() {
    use nil_compiler::{
        SourceProfile, compile_with_profile,
        evaluator::{Value, execute_values},
    };
    use nil_hir::*;
    for seed in 0..1000 {
        let mut r = Random(seed);
        let p = compile_with_profile("(3,i):3=a[b:7]", SourceProfile::ExprV4).unwrap();
        let mut raw = p.hir.program().clone();
        let f = &mut raw.functions[0];
        match r.pick(6) {
            0 => f.parameters[0] = Type::Array(r.pick(512)),
            1 => f.parameters[1] = Type::Bool,
            2 => f.result_type = Type::Array(r.pick(512)),
            3 => f.instructions[1].ty = Type::Array(r.pick(512)),
            4 => {
                f.instructions[1].operation = Operation::Replace {
                    array: ValueId(0),
                    index: ValueId(r.pick(16)),
                    value: ValueId(r.pick(16)),
                }
            }
            _ => {
                f.instructions[1].operation = Operation::Repeat {
                    value: ValueId(r.pick(16)),
                    len: r.pick(512),
                }
            }
        }
        let a = validate(raw.clone());
        let b = validate(raw);
        assert_eq!(a, b);
        if let Ok(p) = a {
            let args = p.program().functions[0]
                .parameters
                .iter()
                .map(|ty| match ty {
                    Type::I64 => Value::I64(0),
                    Type::Bool => Value::Bool(false),
                    Type::Array(n) => Value::array(vec![0; *n]),
                })
                .collect::<Vec<_>>();
            let _ = execute_values(&p, FunctionId(0), &args, nil_fuzz::LIMITS);
            assert_eq!(nil_llvm::emit_llvm(&p), nil_llvm::emit_llvm(&p));
        }
    }
}
