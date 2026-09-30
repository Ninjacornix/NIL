use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, execute},
};
use nil_hir::{Arithmetic, FunctionId};

fn eval(source: &str, args: &[i64]) -> i64 {
    let p = compile_with_profile(source, SourceProfile::ExprV3).unwrap();
    execute(&p.hir, FunctionId(0), args, Limits::default()).unwrap()
}
#[test]
fn v3_reuses_v2_ast_but_explicitly_changes_hir_arithmetic() {
    let text = "=b(20,22)\n2=a+b";
    assert_eq!(
        nil_compiler::parser::parse_with_profile(text, SourceProfile::ExprV2).unwrap(),
        nil_compiler::parser::parse_with_profile(text, SourceProfile::ExprV3).unwrap()
    );
    let a = compile_with_profile(text, SourceProfile::ExprV2).unwrap();
    let b = compile_with_profile(text, SourceProfile::ExprV3).unwrap();
    assert_eq!(a.hir.program().arithmetic, Arithmetic::Checked);
    assert_eq!(b.hir.program().arithmetic, Arithmetic::Wrapping);
    assert_eq!(a.hir.program().functions, b.hir.program().functions);
    assert_eq!(eval(text, &[]), 42);
}
#[test]
fn wrapping_arithmetic_is_deterministic_at_signed_boundaries() {
    for a in [i64::MIN, i64::MIN + 1, -7, -1, 0, 1, 7, i64::MAX] {
        for b in [i64::MIN, -7, -1, 0, 1, 7, i64::MAX] {
            assert_eq!(eval("2=a+b", &[a, b]), a.wrapping_add(b));
            assert_eq!(eval("2=a-b", &[a, b]), a.wrapping_sub(b));
            assert_eq!(eval("2=a*b", &[a, b]), a.wrapping_mul(b));
            if b != 0 {
                assert_eq!(eval("2=a/b", &[a, b]), a.wrapping_div(b));
            }
        }
    }
    assert_eq!(eval("2=a<b?a:b", &[i64::MIN, i64::MAX]), i64::MIN);
}
#[test]
fn zero_division_traps_and_inactive_branches_are_lazy() {
    let p = compile_with_profile("2=a/b", SourceProfile::ExprV3).unwrap();
    let err = execute(&p.hir, FunctionId(0), &[1, 0], Limits::default()).unwrap_err();
    assert_eq!(err.code, "E009");
    assert_eq!(err.message, "division by zero");
    assert_eq!(eval("=1==1?42:1/0", &[]), 42);
    let p = compile_with_profile("2=a+b", SourceProfile::ExprV2).unwrap();
    assert_eq!(
        execute(&p.hir, FunctionId(0), &[i64::MAX, 1], Limits::default())
            .unwrap_err()
            .code,
        "E009"
    );
}
#[test]
fn v3_type_checking_and_canonical_spelling_remain_strict() {
    for source in ["0=42", "1=b", "=1+true", "=b()", "=01", "=loop(0;0>1;0;0)"] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV3).is_err(),
            "{source}"
        );
    }
}
