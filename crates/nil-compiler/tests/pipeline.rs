use nil_compiler::{
    compile, dump,
    evaluator::{Limits, execute},
    hir::*,
    parser,
};

fn run(source: &str, label: u32, args: &[i64]) -> Result<i64, Diagnostic> {
    let program = compile(source)?;
    execute(
        &program.hir,
        program.function(label).unwrap(),
        args,
        Limits::default(),
    )
}
fn arithmetic(op: &str, a: i64, b: i64) -> String {
    format!("fn 0 -> i64\nconst {a}\nconst {b}\n{op} 0 1\nret 2\nend")
}

#[test]
fn source_to_ast_to_checked_hir_to_execution() {
    let ast = parser::parse(include_str!("../../../examples/add.nil")).unwrap();
    assert_eq!(ast.functions.len(), 2);
    let program = nil_compiler::lower(ast).unwrap();
    assert_eq!(
        execute(
            &program.hir,
            program.function(0).unwrap(),
            &[],
            Limits::default()
        )
        .unwrap(),
        42
    );
    assert_eq!(
        execute(
            &program.hir,
            program.function(1).unwrap(),
            &[20, 22],
            Limits::default()
        )
        .unwrap(),
        42
    );
}

#[test]
fn parameters_constants_and_signed_boundaries() {
    let source = "fn 42 i64 -> i64\nret 0\nend";
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(run(source, 42, &[value]).unwrap(), value);
        assert_eq!(
            run(&format!("fn 0 -> i64\nconst {value}\nret 0\nend"), 0, &[]).unwrap(),
            value
        );
    }
}

#[test]
fn all_arithmetic_and_truncating_division() {
    for (op, a, b, expected) in [
        ("add", 20, 22, 42),
        ("sub", 20, 22, -2),
        ("mul", -6, 7, -42),
        ("div", -7, 2, -3),
        ("div", 7, -2, -3),
        ("div", -7, -2, 3),
    ] {
        assert_eq!(run(&arithmetic(op, a, b), 0, &[]).unwrap(), expected);
    }
}

#[test]
fn arithmetic_traps_are_structured() {
    for (op, a, b) in [
        ("add", i64::MAX, 1),
        ("sub", i64::MIN, 1),
        ("mul", i64::MAX, 2),
        ("div", i64::MIN, -1),
        ("div", 1, 0),
    ] {
        let e = run(&arithmetic(op, a, b), 0, &[]).unwrap_err();
        assert_eq!(e.code, "E009");
        assert_eq!(e.phase, Phase::Execute);
        assert!(e.span.is_some());
    }
}

#[test]
fn nested_forward_calls_preserve_caller_values_and_results() {
    let source = "fn 10 -> i64\nconst 5\ncall 30 0\ncall 30 1\nadd 0 2\nret 3\nend\nfn 30 i64 -> i64\ncall 20 0\nconst 1\nadd 1 2\nret 3\nend\nfn 20 i64 -> i64\nmul 0 0\nret 1\nend";
    assert_eq!(run(source, 10, &[]).unwrap(), 682);
}

#[test]
fn zero_argument_calls() {
    assert_eq!(
        run(
            "fn 0 -> i64\ncall 9\nret 0\nend\nfn 9 -> i64\nconst 42\nret 0\nend",
            0,
            &[]
        )
        .unwrap(),
        42
    );
}

#[test]
fn whitespace_crlf_and_final_eof() {
    assert_eq!(
        run("\r\n\tfn 0 -> i64\r\n\tconst 42\r\n\nret 0\r\nend", 0, &[]).unwrap(),
        42
    );
}

#[test]
fn malformed_programs_are_rejected() {
    for source in [
        "",
        "   \n",
        "end",
        "fn",
        "fn 0 i64",
        "fn 0 ->",
        "fn 0 -> i64 extra",
        "fn 0 -> i64\nend",
        "fn 0 -> i64\nret 0",
        "fn 0 -> i64\nconst\nret 0\nend",
        "fn 0 -> i64\nconst 1 extra\nret 0\nend",
        "fn 0 -> i64\nadd 0\nret 0\nend",
        "fn 0 -> i64\ncall\nret 0\nend",
        "fn 0 -> i64\nret 0 1\nend",
        "fn 0 i64 -> i64\nret 0\nret 0\nend",
        "fn 0 i64 -> i64\nret 0\nconst 1\nend",
        "fn 0 i64 -> i64\nret 0\nend extra",
        "fn 0 -> i64\nλ\nend",
        "fn 0 -> i64\n# comment\nend",
        "fn 0 -> i64\nconst 1\nret 0\nend\ntrailing",
    ] {
        assert_eq!(compile(source).unwrap_err().code, "E001", "{source:?}");
    }
}

#[test]
fn rejects_noncanonical_and_out_of_range_numbers() {
    for value in [
        "+1",
        "01",
        "-0",
        "-01",
        "9223372036854775808",
        "-9223372036854775809",
        "1.0",
        "١",
        "--1",
    ] {
        assert_eq!(
            compile(&format!("fn 0 -> i64\nconst {value}\nret 0\nend"))
                .unwrap_err()
                .code,
            "E001"
        );
    }
    for id in ["-1", "+1", "01", "4294967296", "x"] {
        assert_eq!(
            compile(&format!("fn {id} i64 -> i64\nret 0\nend"))
                .unwrap_err()
                .code,
            "E001"
        );
    }
}

#[test]
fn unsupported_types_in_both_signature_positions() {
    for source in [
        "fn 0 bool -> i64\nret 0\nend",
        "fn 0 i64 -> str\nret 0\nend",
    ] {
        let e = compile(source).unwrap_err();
        assert_eq!(e.code, "E002");
        assert_eq!(e.expected.as_deref(), Some("i64"));
    }
}

#[test]
fn semantic_rejections_include_unused_functions() {
    for (source, code) in [
        (
            "fn 0 i64 -> i64\nret 0\nend\nfn 0 i64 -> i64\nret 0\nend",
            "E003",
        ),
        ("fn 0 -> i64\ncall 8\nret 0\nend", "E004"),
        ("fn 0 -> i64\nconst 1\nadd 0 1\nret 2\nend", "E005"),
        ("fn 0 -> i64\nret 0\nend", "E005"),
        (
            "fn 0 -> i64\ncall 1\nret 0\nend\nfn 1 i64 -> i64\nret 0\nend",
            "E006",
        ),
        ("fn 0 i64 -> i64\ncall 0 0 0\nret 1\nend", "E006"),
        (
            "fn 0 -> i64\nconst 42\nret 0\nend\nfn 1 -> i64\nret 0\nend",
            "E005",
        ),
    ] {
        let e = compile(source).unwrap_err();
        assert_eq!(e.code, code);
        assert_eq!(e.phase, Phase::Check);
    }
}

#[test]
fn recursion_hits_depth_limit_without_host_recursion() {
    let p = compile("fn 0 -> i64\ncall 0\nret 0\nend").unwrap();
    let e = execute(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            steps: 1000,
            call_depth: 20,
        },
    )
    .unwrap_err();
    assert_eq!(e.code, "E008");
    assert!(e.message.contains("depth"));
}

#[test]
fn mutual_recursion_hits_fuel_limit() {
    let p = compile("fn 0 -> i64\ncall 1\nret 0\nend\nfn 1 -> i64\ncall 0\nret 0\nend").unwrap();
    let e = execute(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            steps: 10,
            call_depth: 20,
        },
    )
    .unwrap_err();
    assert_eq!(e.code, "E008");
    assert!(e.message.contains("budget"));
}

#[test]
fn instruction_budget_counts_return_and_entry_depth() {
    let p = compile("fn 0 -> i64\nconst 42\nret 0\nend").unwrap();
    for steps in [0, 1] {
        assert_eq!(
            execute(
                &p.hir,
                FunctionId(0),
                &[],
                Limits {
                    steps,
                    call_depth: 1
                }
            )
            .unwrap_err()
            .code,
            "E008"
        );
    }
    assert_eq!(
        execute(
            &p.hir,
            FunctionId(0),
            &[],
            Limits {
                steps: 2,
                call_depth: 1
            }
        )
        .unwrap(),
        42
    );
    assert_eq!(
        execute(
            &p.hir,
            FunctionId(0),
            &[],
            Limits {
                steps: 2,
                call_depth: 0
            }
        )
        .unwrap_err()
        .code,
        "E008"
    );
}

#[test]
fn evaluator_rejects_bad_entry_and_arguments() {
    let p = compile("fn 0 i64 -> i64\nret 0\nend").unwrap();
    assert_eq!(
        execute(&p.hir, FunctionId(100), &[], Limits::default())
            .unwrap_err()
            .code,
        "E004"
    );
    let e = execute(&p.hir, FunctionId(0), &[], Limits::default()).unwrap_err();
    assert_eq!(e.code, "E006");
    assert_eq!(e.expected.as_deref(), Some("1"));
    assert_eq!(e.actual.as_deref(), Some("0"));
}

#[test]
fn source_size_is_bounded() {
    assert_eq!(
        compile(&" ".repeat(parser::MAX_SOURCE_BYTES + 1))
            .unwrap_err()
            .code,
        "E008"
    );
}

#[test]
fn deterministic_lowering_and_debug_golden() {
    let source = include_str!("../../../examples/add.nil");
    let a = compile(source).unwrap();
    let b = compile(source).unwrap();
    assert_eq!(a.hir, b.hir);
    assert_eq!(dump(&a.hir), include_str!("fixtures/add.hir"));
    // Source labels are not semantic function identities.
    let renamed = source
        .replace("fn 0", "fn 8")
        .replace("fn 1", "fn 9")
        .replace("call 1", "call 9");
    assert_eq!(dump(&a.hir), dump(&compile(&renamed).unwrap().hir));
}

#[test]
fn diagnostic_golden_uses_byte_spans() {
    let e = compile("fn 0 -> i64\nconst 1\nadd 0 1\nret 2\nend\n").unwrap_err();
    assert_eq!(format!("{e}\n"), include_str!("fixtures/invalid-value.txt"));
}
