use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{FunctionId, Operation, Span};
fn eval(source: &str) -> Value {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap()
}
#[test]
fn remainder_is_signed_truncating_and_minimum_edge_is_zero() {
    for (s, n) in [
        ("=-7%3", -1),
        ("=7%-3", 1),
        ("=-7%-3", -1),
        ("=i64min%-1", 0),
    ] {
        assert_eq!(eval(s), Value::I64(n));
    }
    assert_eq!(eval(":u128=u128max%2u128"), Value::U128(1));
}
#[test]
fn integer_bits_and_shifts_preserve_width_and_signedness() {
    assert_eq!(eval("=-8>>2"), Value::I64(-2));
    assert_eq!(eval(":u64=u64max>>63"), Value::U64(1));
    assert_eq!(eval(":u128=1u128<<127"), Value::U128(1 << 127));
    assert_eq!(eval("=~0"), Value::I64(-1));
    assert_eq!(eval("=0b1010^0x3|1"), Value::I64(9));
}
#[test]
fn scalar_primitives_preserve_single_evaluation() {
    assert_eq!(eval("=!min(4,9)+!max(4,9)+!abs(-3)"), Value::I64(16));
    assert_eq!(eval(":u64=!abs(u64max)"), Value::U64(u64::MAX));
}
#[test]
fn invalid_operator_types_are_static_errors() {
    for s in [
        ":f64=1.0%2.0",
        ":u64=1u64&1",
        ":u128=1u128<<1u64",
        ":b=1&&true",
        "=~true",
        "=!sqrt(4)",
        "=!min(1,1u64)",
    ] {
        assert_eq!(
            compile_with_profile(s, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E007",
            "{s}"
        );
    }
}
#[test]
fn a_scalar_given_to_the_one_argument_reduction_is_a_type_error() {
    // Not E006: one argument is a legal arity since ADR 055 step 3.
    for s in ["=!min(1)", "=!max(1)", "=!sum(1)"] {
        let d = compile_with_profile(s, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(d.code, "E007", "{s}");
    }
}
#[test]
fn scalar_call_arity_errors_have_the_complete_call_span() {
    // ADR 055 step 3 made one argument a valid arity for !min/!max, so
    // `!min(1)` is now a type error rather than an arity error: a scalar is not
    // a sequence to reduce. It is asserted separately below.
    for s in ["=!sqrt()", "=!max(1,2,3)"] {
        let d = compile_with_profile(s, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(d.code, "E006");
        assert_eq!(
            d.span,
            Some(Span {
                start: 1,
                end: s.len()
            })
        );
    }
}
#[test]
fn constants_have_explicit_types_and_radix_ranges() {
    assert_eq!(eval("=i64min"), Value::I64(i64::MIN));
    assert_eq!(eval("=i64max"), Value::I64(i64::MAX));
    assert_eq!(eval(":u64=u64min|u64max"), Value::U64(u64::MAX));
    assert_eq!(eval(":u128=u128max"), Value::U128(u128::MAX));
    assert_eq!(eval("=-0x8000000000000000"), Value::I64(i64::MIN));
    assert_eq!(eval(":u64=0xffffffffffffffffu64"), Value::U64(u64::MAX));
}
#[test]
fn malformed_radix_literals_and_reserved_binding_names_have_spans() {
    for s in [
        "=0x",
        "=0b2",
        "=0x00",
        "=0xFF",
        "=0x8000000000000000",
        ":u64=-0x1u64",
        "=inf=1;1",
        "=x=;x",
        "=x=1 x",
    ] {
        let d = compile_with_profile(s, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(d.code, "E001", "{s}");
        assert!(d.span.is_some());
    }
}
#[test]
fn integer_exponent_proposal_does_not_repurpose_existing_floats() {
    assert_eq!(eval(":f64=1e9"), Value::F64(1e9f64.to_bits()));
    assert_eq!(
        compile_with_profile("=1e9i", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E001"
    );
}
#[test]
fn bindings_are_ssa_names_not_runtime_operations() {
    let p = compile_with_profile("=x=20;y=x+2;y+y", SourceProfile::ExprV5).unwrap();
    assert_eq!(p.hir.program().functions[0].instructions.len(), 4);
    assert!(matches!(
        p.hir.program().functions[0].instructions[2].operation,
        Operation::Binary { .. }
    ));
    assert_eq!(eval("=x=20;y=x+2;y+y"), Value::I64(44));
}
#[test]
fn binding_shadow_initializers_see_the_previous_value_and_restore_on_exit() {
    assert_eq!(eval("=x=7;(x=x+1;x)+x"), Value::I64(15));
    assert_eq!(eval("=x=7;x=x+1;x"), Value::I64(8));
    assert_eq!(
        compile_with_profile("=(x=7;x)+x", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E005"
    );
}
#[test]
fn bindings_scope_over_loop_state_lists_and_each_steps() {
    assert_eq!(eval("=x=99;@(x,0;b<3;x=a+1;x,b+1;a)+x"), Value::I64(201));
    assert_eq!(eval("=!each([2,3],0;item=b+1;c+item;a)"), Value::I64(7));
    assert_eq!(eval("=@(0,0;b<3;b=a+1;b,b;a)"), Value::I64(3));
}
#[test]
fn state_regions_remain_closed_and_bindings_do_not_leak_between_them() {
    for s in ["=x=7;@(0;a<x;a+1;a)", "=@(0;a<2;x=a+1;x;x)"] {
        assert_eq!(
            compile_with_profile(s, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E005"
        );
    }
}
#[test]
fn logical_precedence_and_negated_calls_are_unambiguous() {
    assert_eq!(eval(":b=true||false&&false"), Value::Bool(true));
    assert_eq!(eval(":b=!(b())\n:b=false"), Value::Bool(true));
    assert_eq!(eval("=1+2<<2"), Value::I64(12));
}
#[test]
fn new_surface_is_rejected_by_previous_profiles() {
    for profile in [SourceProfile::ExprV3, SourceProfile::ExprV4] {
        for s in ["=7%3", "=1<<2", "=x=1;x", "=i64max"] {
            assert!(
                compile_with_profile(s, profile).is_err(),
                "{profile:?}: {s}"
            );
        }
    }
}

#[test]
fn hexadecimal_e_digit_does_not_absorb_adjacent_arithmetic() {
    assert_eq!(eval("=0xe+1"), Value::I64(15));
    assert_eq!(eval("=0xe-1"), Value::I64(13));
}

#[test]
fn wide_loop_positional_labels_keep_precedence_over_float_constants() {
    for label in ["inf", "nan"] {
        let index = label
            .bytes()
            .fold(0usize, |n, b| n * 26 + (b - b'a') as usize + 1)
            - 1;
        let arity = index + 1;
        let initial = vec!["42"; arity].join(",");
        let source = format!("=@({initial};false;{initial};{label})");
        assert_eq!(eval(&source), Value::I64(42));
    }
}
