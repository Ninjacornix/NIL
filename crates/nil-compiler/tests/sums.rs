//! Sum types, recursive declarations and the pattern-match expression (ADR 050).
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{Diagnostic, FunctionId};

fn check(source: &str) -> Result<nil_compiler::CompiledProgram, Diagnostic> {
    compile_with_profile(source, SourceProfile::ExprV5)
}
fn execute(source: &str, args: &[Value]) -> Result<Value, Diagnostic> {
    execute_values(
        &check(source).unwrap().hir,
        FunctionId(0),
        args,
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
    )
}
fn fails(source: &str) -> Diagnostic {
    check(source).expect_err("expected a diagnostic")
}

#[test]
fn a_nullary_variant_needs_no_parentheses_and_a_payload_variant_round_trips() {
    // Depth of a one-node tree: selects the payload arm and binds positionally.
    let source = "sum T(N(i,T,T),L)\n1=b(N(a,L,L))\n(T)=?a(N(v,_,_):v,L:-1)\n";
    assert_eq!(execute(source, &[Value::I64(7)]).unwrap(), Value::I64(7));
    // The nullary arm is selected for a bare variant.
    let bare = "sum T(N(i,T,T),L)\n=b(L)\n(T)=?a(N(v,_,_):v,L:-1)\n";
    assert_eq!(execute(bare, &[]).unwrap(), Value::I64(-1));
}

#[test]
fn a_recursive_sum_sums_a_list_built_by_recursion() {
    // Cons(n, Cons(n-1, ... Nil)) then fold it back down.
    let source = "sum L(C(i,L),E)\n\
                  1=c(b(a))\n\
                  1:L=a<=0?E:C(a,b(a-1))\n\
                  (L)=?a(C(h,t):h+c(t),E:0)\n";
    assert_eq!(execute(source, &[Value::I64(4)]).unwrap(), Value::I64(10));
    assert_eq!(execute(source, &[Value::I64(0)]).unwrap(), Value::I64(0));
    assert_eq!(
        execute(source, &[Value::I64(100)]).unwrap(),
        Value::I64(5050)
    );
}

#[test]
fn the_balanced_tree_witness_program_checks_and_runs() {
    // The ADR 050 token witness: a recursive height-or-unbalanced check with
    // no integer links and no offset arithmetic.
    let source = "sum T(N(i,T,T),L)\n\
                  1=c(b(a))\n\
                  1:T=a<=0?L:N(a,b(a-1),b(a-1))\n\
                  (T)=?a(N(_,l,r):(x=c(l);y=c(r);x<0?-1:y<0?-1:!abs(x-y)>1?-1:1+!max(x,y)),L:0)\n";
    // A perfectly balanced tree of depth 5 has height 5.
    assert_eq!(execute(source, &[Value::I64(5)]).unwrap(), Value::I64(5));
    assert_eq!(execute(source, &[Value::I64(0)]).unwrap(), Value::I64(0));
}

#[test]
fn an_unbalanced_tree_reports_the_sentinel_free_failure() {
    // Left spine only: height difference exceeds one as soon as depth is 2.
    let source = "sum T(N(i,T,T),L)\n\
                  1=c(b(a))\n\
                  1:T=a<=0?L:N(a,b(a-1),L)\n\
                  (T)=?a(N(_,l,r):(x=c(l);y=c(r);x<0?-1:y<0?-1:!abs(x-y)>1?-1:1+!max(x,y)),L:0)\n";
    assert_eq!(execute(source, &[Value::I64(1)]).unwrap(), Value::I64(1));
    assert_eq!(execute(source, &[Value::I64(3)]).unwrap(), Value::I64(-1));
}

#[test]
fn mutually_recursive_sums_may_be_declared_in_either_order() {
    // A refers forward to B, declared later.
    let source = "sum A(MkA(B),EndA)\n\
                  sum B(MkB(i))\n\
                  1=b(MkA(MkB(a)))\n\
                  (A)=?a(MkA(x):?x(MkB(n):n*2),EndA:0)\n";
    assert_eq!(execute(source, &[Value::I64(21)]).unwrap(), Value::I64(42));
}

#[test]
fn a_record_field_may_hold_a_sum_declared_later() {
    let source = "record W(t:T)\n\
                  sum T(N(i),L)\n\
                  1=b(W(N(a)))\n\
                  (W)=(x=a.t;?x(N(v):v,L:0))\n";
    assert_eq!(execute(source, &[Value::I64(9)]).unwrap(), Value::I64(9));
}

#[test]
fn arms_are_lazy_so_an_unselected_arm_never_traps() {
    // The unselected arm divides by zero. Selecting the nullary arm must not
    // evaluate it, so the program returns instead of trapping.
    let lazy = "sum T(N(i),L)\n=b(L)\n(T)=?a(N(v):v/0,L:5)\n";
    assert_eq!(execute(lazy, &[]).unwrap(), Value::I64(5));
    // Selecting the payload arm does trap, which proves the arm really runs
    // and that the result above came from laziness, not from a folded away arm.
    let traps = "sum T(N(i),L)\n=b(N(1))\n(T)=?a(N(v):v/0,L:5)\n";
    assert!(execute(traps, &[]).is_err());
}

#[test]
fn a_non_exhaustive_match_names_the_uncovered_variants() {
    let e = fails("sum T(N(i),L)\n(T)=?a(N(v):v)\n");
    assert_eq!(e.code, "E025");
    assert!(e.message.contains('L'), "{}", e.message);
    assert!(e.span.is_some());
}

#[test]
fn a_duplicate_arm_is_an_unreachable_arm() {
    let e = fails("sum T(N(i),L)\n(T)=?a(N(v):v,N(w):w,L:0)\n");
    assert_eq!(e.code, "E026");
    assert!(e.message.contains("already covered"), "{}", e.message);
    assert!(e.span.is_some());
}

#[test]
fn matching_a_scrutinee_from_another_sum_is_rejected_exactly() {
    // Variant names resolve across the whole group, so the sum carried by the
    // tag read is what makes this exact rather than silently wrong.
    let e = fails("sum A(P(i),Q)\nsum B(R(i),S)\n(A)=?a(R(v):v,S:0)\n");
    assert_eq!(e.code, "E007");
    assert!(e.span.is_some());
}

#[test]
fn a_sum_with_no_finite_values_is_rejected() {
    assert_eq!(fails("sum X(OnlyX(X))\n=1\n").code, "E023");
    assert_eq!(fails("sum A(MkA(B))\nsum B(MkB(A))\n=1\n").code, "E023");
}

#[test]
fn duplicate_names_and_unknown_variants_are_rejected() {
    assert_eq!(fails("sum A(P(i),Q)\nsum A(R(i))\n=1\n").code, "E023");
    // A variant name may not repeat anywhere in the group.
    assert_eq!(fails("sum A(P(i),Q)\nsum B(P(i))\n=1\n").code, "E023");
    // A sum may not share a name with a record.
    assert_eq!(fails("record S(x:i)\nsum S(P(i))\n=1\n").code, "E023");
    assert_eq!(
        fails("sum T(N(i),L)\n(T)=?a(Missing(v):v,L:0)\n").code,
        "E023"
    );
}

#[test]
fn variant_arity_is_checked_at_construction_and_in_bindings() {
    assert_eq!(fails("sum T(N(i,i),L)\n:T=N(1)\n").code, "E006");
    assert_eq!(fails("sum T(N(i,i),L)\n(T)=?a(N(x):x,L:0)\n").code, "E006");
    // A payload variant cannot be spelled bare in an arm.
    assert_eq!(fails("sum T(N(i),L)\n(T)=?a(N:1,L:0)\n").code, "E006");
}

#[test]
fn lowercase_sum_and_variant_names_are_rejected() {
    assert_eq!(fails("sum t(N(i),L)\n=1\n").code, "E023");
    assert_eq!(fails("sum T(n(i),L)\n=1\n").code, "E023");
}

#[test]
fn a_wildcard_binds_nothing_and_cannot_be_read() {
    let source = "sum T(N(i,i),L)\n2=b(N(a,b))\n(T)=?a(N(_,y):y,L:0)\n";
    assert_eq!(
        execute(source, &[Value::I64(1), Value::I64(2)]).unwrap(),
        Value::I64(2)
    );
    // `_` is not a readable name inside the arm.
    assert!(check("sum T(N(i,i),L)\n(T)=?a(N(_,y):_+y,L:0)\n").is_err());
}

#[test]
fn declarations_must_precede_functions() {
    assert!(check("=1\nsum T(N(i),L)\n").is_err());
}

#[test]
fn existing_record_programs_are_unchanged_by_the_new_syntax() {
    // `sum` is only a declaration keyword in the application profile, and the
    // record surface keeps its meaning.
    let source = "record P(x:i,y:i)\n2=b(P(a,b))\n(P)=a.x+a.y\n";
    assert_eq!(
        execute(source, &[Value::I64(20), Value::I64(22)]).unwrap(),
        Value::I64(42)
    );
}

#[test]
fn deep_list_depth_is_bounded_by_the_call_guard_not_by_the_drop() {
    // ADR 050 requires a non-recursive drop for a 10^6-node list. That cannot
    // be exercised through source recursion on the reference path: the call
    // depth guard fires first, which is an ordinary resource outcome rather
    // than a drop defect. Reaching 10^6 nodes needs an iterative builder under
    // native execution, which is still owed.
    let source = "sum L(C(i,L),E)\n1=c(b(a))\n1:L=a<=0?E:C(a,b(a-1))\n(L)=?a(C(h,t):h+c(t),E:0)\n";
    assert_eq!(
        execute(source, &[Value::I64(2_000)]).unwrap(),
        Value::I64(2_001_000)
    );
    assert_eq!(
        execute(source, &[Value::I64(20_000)]).unwrap_err().code,
        "E008"
    );
}
