use nil_compiler::{
    SourceProfile, compile_with_profile, dump,
    evaluator::{Limits, execute},
};

fn evaluate(source: &str, profile: SourceProfile, args: &[i64]) -> Result<i64, String> {
    let program = compile_with_profile(source, profile).map_err(|e| e.to_string())?;
    execute(
        &program.hir,
        program.function(0).unwrap(),
        args,
        Limits::default(),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn all_experiment_profiles_lower_to_identical_hir() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/samples");
    for name in [
        "affine",
        "squares",
        "polynomial",
        "constant",
        "identity",
        "add",
        "difference",
        "product",
        "weighted",
        "nested",
        "grouped",
        "quotient",
    ] {
        let mut ir = Vec::new();
        for (suffix, profile) in [
            ("v0", SourceProfile::ExprV0),
            ("v1", SourceProfile::ExprV1),
            ("v2", SourceProfile::ExprV2),
        ] {
            let source = std::fs::read_to_string(root.join(format!("{name}.{suffix}"))).unwrap();
            let program = compile_with_profile(&source, profile).unwrap();
            ir.push(dump(&program.hir));
        }
        assert_eq!(ir[0], ir[1], "{name}: named headers changed semantics");
        assert_eq!(ir[0], ir[2], "{name}: positional headers changed semantics");
    }
}

#[test]
fn forward_calls_nested_calls_and_parameter_function_names_are_unambiguous() {
    assert_eq!(
        evaluate("2=b(a)+b(b)\n1=a*a", SourceProfile::ExprV2, &[3, 4]).unwrap(),
        25
    );
    assert_eq!(
        evaluate("a,b=b(a)+b(b)\nx=x*x", SourceProfile::ExprV1, &[3, 4]).unwrap(),
        25
    );
    assert_eq!(
        evaluate("=b()\n=42", SourceProfile::ExprV2, &[]).unwrap(),
        42
    );
}

#[test]
fn multi_letter_references_and_parameters_work() {
    let mut source = "=aa()\n".to_string();
    for _ in 1..26 {
        source.push_str("=0\n");
    }
    source.push_str("=42");
    assert_eq!(evaluate(&source, SourceProfile::ExprV2, &[]).unwrap(), 42);
    let args: Vec<i64> = (0..27).collect();
    assert_eq!(evaluate("27=aa", SourceProfile::ExprV2, &args).unwrap(), 26);
}

#[test]
fn malformed_headers_calls_and_parameters_are_rejected() {
    for source in [
        "",
        "0=42",
        "01=a",
        "4097=a",
        "1=b",
        "2=a+c",
        "1=A",
        "1=a()",
        "1=zzzzzzzzzzzzzzzzzz()",
        "1:i64=a",
        "1=a # comment",
        "1=01",
        "1=-0",
        "1=a\r1=a",
        "1=(a+)",
        "1=b(a,a)\n1=a",
    ] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV2).is_err(),
            "{source}"
        );
    }
    for source in ["x,x=x", "x=y", "x:i64=x", "x=x\nx=b()", "x=x\ny=a(y,y)"] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV1).is_err(),
            "{source}"
        );
    }
}

#[test]
fn compact_profiles_preserve_traps_limits_and_signed_division() {
    assert_eq!(
        evaluate("2=a/b", SourceProfile::ExprV2, &[-9, 2]).unwrap(),
        -4
    );
    for (source, args) in [
        ("2=a+b", vec![i64::MAX, 1]),
        ("2=a/b", vec![1, 0]),
        ("2=a/b", vec![i64::MIN, -1]),
        ("1=a(a)", vec![0]),
    ] {
        assert!(evaluate(source, SourceProfile::ExprV2, &args).is_err());
    }
    assert_eq!(
        evaluate("\r\n 2=a+b\r\n", SourceProfile::ExprV2, &[20, 22]).unwrap(),
        42
    );
}

#[test]
fn positional_diagnostics_retain_original_byte_spans() {
    let error = compile_with_profile("2=a+λ", SourceProfile::ExprV2).unwrap_err();
    assert_eq!(
        error.span,
        Some(nil_compiler::hir::Span { start: 4, end: 6 })
    );
    assert_eq!(SourceProfile::default(), SourceProfile::ExprV0);
}
