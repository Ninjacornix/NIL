use nil_compiler::{
    SourceProfile, compile, compile_with_profile, dump,
    evaluator::{Limits, execute},
};

const PAIRS: [(&str, &str, &[i64], i64); 3] = [
    (
        include_str!("../../../benchmarks/paired/samples/affine.nil"),
        include_str!("../../../benchmarks/paired/samples/affine.expr.nil"),
        &[20, 22],
        124,
    ),
    (
        include_str!("../../../benchmarks/paired/samples/squares.nil"),
        include_str!("../../../benchmarks/paired/samples/squares.expr.nil"),
        &[3, 4],
        25,
    ),
    (
        include_str!("../../../benchmarks/paired/samples/polynomial.nil"),
        include_str!("../../../benchmarks/paired/samples/polynomial.expr.nil"),
        &[5],
        36,
    ),
];

fn expr(source: &str) -> nil_compiler::CompiledProgram {
    compile_with_profile(source, SourceProfile::ExprV0).unwrap()
}

#[test]
fn expression_fixtures_lower_to_same_hir_as_lines() {
    for (lines, expression, args, expected) in PAIRS {
        let old = compile_with_profile(lines, SourceProfile::LinesV0).unwrap();
        let new = expr(expression);
        assert_eq!(dump(&old.hir), dump(&new.hir));
        assert_eq!(
            execute(&new.hir, new.function(0).unwrap(), args, Limits::default()).unwrap(),
            expected
        );
    }
}

#[test]
fn inferred_types_precedence_and_negative_literals() {
    let program = expr("f0(x,y)=x+y*2\nf1(x)=-9223372036854775808\n");
    let entry = program.function(0).unwrap();
    assert_eq!(
        execute(&program.hir, entry, &[3, 4], Limits::default()).unwrap(),
        11
    );
    let min = program.function(1).unwrap();
    assert_eq!(
        execute(&program.hir, min, &[0], Limits::default()).unwrap(),
        i64::MIN
    );
    let left_associative = expr("f0(x)=x-3-2");
    assert_eq!(
        execute(
            &left_associative.hir,
            left_associative.function(0).unwrap(),
            &[10],
            Limits::default()
        )
        .unwrap(),
        5
    );
}

#[test]
fn expression_calls_resolve_forward_and_preserve_traps() {
    let program = expr("f0(x)=f1(x)/2\nf1(x)=x*x\n");
    assert_eq!(
        execute(
            &program.hir,
            program.function(0).unwrap(),
            &[5],
            Limits::default()
        )
        .unwrap(),
        12
    );
    let trap = expr("f0(x)=x/0");
    let error = execute(
        &trap.hir,
        trap.function(0).unwrap(),
        &[2],
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "E009");
}

#[test]
fn malformed_expression_profiles_fail_with_diagnostics() {
    for (source, code) in [
        ("f0(x)=x+", "E001"),
        ("f0(x)=x*2 garbage", "E001"),
        ("f0(x,x)=x", "E001"),
        ("f0(x:u32)=x", "E002"),
        ("f0(x)=z", "E005"),
        ("f0(x)=f1(x)", "E004"),
        ("f0(x)=x\nf0(y)=y", "E003"),
        ("f0(x)=01", "E001"),
        ("f0(x)=-0", "E001"),
        ("f0(x)=9223372036854775808", "E001"),
    ] {
        let error = compile_with_profile(source, SourceProfile::ExprV0).unwrap_err();
        assert_eq!(error.code, code, "{source}");
    }
}

#[test]
fn nested_expression_limit_is_enforced() {
    let source = format!("f0(x)={}x{}", "(".repeat(129), ")".repeat(129));
    let error = compile_with_profile(&source, SourceProfile::ExprV0).unwrap_err();
    assert_eq!(error.code, "E001");
    assert!(error.message.contains("nesting limit"));
}

#[test]
fn expression_profile_is_the_default() {
    assert!(compile("f0(x)=x").is_ok());
    assert!(compile_with_profile("fn 0 i64 -> i64\nret 0\nend", SourceProfile::ExprV0).is_err());
}

#[test]
fn public_parser_defaults_to_expr_and_exposes_lines_explicitly() {
    assert!(nil_compiler::parser::parse("f0(x)=x+1").is_ok());
    assert!(nil_compiler::parser::parse_lines("fn 0 i64 -> i64\nret 0\nend").is_ok());
    assert!(nil_compiler::parser::parse("fn 0 i64 -> i64\nret 0\nend").is_err());
    assert!(nil_compiler::parser::parse_lines("f0(x)=x+1").is_err());
}
