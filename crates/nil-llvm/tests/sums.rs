//! Native sum lowering at O0 and O2 (ADR 050).
//!
//! Every case is also checked against the reference evaluator, so native
//! results are never trusted alone.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use nil_llvm::{Optimization, Options};
use std::process::Output;

fn native(source: &str, args: &[&str], optimization: Optimization) -> Output {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    nil_llvm::run_arguments(
        &p.hir,
        Options {
            optimization,
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
            ..Default::default()
        },
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
}
fn reference(source: &str, values: &[Value]) -> Value {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    execute_values(
        &p.hir,
        FunctionId(0),
        values,
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Limits::default()
        },
    )
    .unwrap()
}
/// Asserts the reference evaluator and both native levels all agree.
fn agrees(source: &str, args: &[&str], values: &[Value], expected: i64) {
    assert_eq!(
        reference(source, values),
        Value::I64(expected),
        "reference: {source}"
    );
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = native(source, args, optimization);
        assert!(
            out.status.success(),
            "{source} {optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            expected.to_string(),
            "{source} {optimization:?}"
        );
    }
}

const LIST: &str =
    "sum L(C(i,L),E)\n1=c(b(a))\n1:L=a<=0?E:C(a,b(a-1))\n(L)=?a(C(h,t):h+c(t),E:0)\n";
const TREE: &str = "sum T(N(i,T,T),L)\n1=c(b(a))\n1:T=a<=0?L:N(a,b(a-1),b(a-1))\n(T)=?a(N(_,l,r):(x=c(l);y=c(r);x<0?-1:y<0?-1:!abs(x-y)>1?-1:1+!max(x,y)),L:0)\n";

#[test]
fn every_variant_shape_agrees_across_reference_and_both_levels() {
    // Nullary only, one payload slot, several slots, and a sum-typed slot.
    agrees("sum T(A,B)\n=b(A)\n(T)=?a(A:7,B:9)\n", &[], &[], 7);
    agrees("sum T(A,B)\n=b(B)\n(T)=?a(A:7,B:9)\n", &[], &[], 9);
    agrees("sum T(P(i),Q)\n=b(P(5))\n(T)=?a(P(v):v,Q:0)\n", &[], &[], 5);
    agrees(
        "sum T(P(i,i,i),Q)\n=b(P(1,2,4))\n(T)=?a(P(x,y,z):x+y+z,Q:0)\n",
        &[],
        &[],
        7,
    );
    // A sum-typed payload slot, read one level down.
    agrees(
        "sum T(W(T),L)\n=b(W(W(L)))\n(T)=?a(W(x):?x(W(y):2,L:1),L:0)\n",
        &[],
        &[],
        2,
    );
    // A nullary variant is unboxed, so its tag must still round-trip.
    agrees(
        "sum T(A,B,C,D)\n=b(D)\n(T)=?a(A:0,B:1,C:2,D:3)\n",
        &[],
        &[],
        3,
    );
}

#[test]
fn recursive_values_fold_identically_at_both_levels() {
    agrees(LIST, &["4"], &[Value::I64(4)], 10);
    agrees(LIST, &["100"], &[Value::I64(100)], 5050);
    agrees(TREE, &["1"], &[Value::I64(1)], 1);
    agrees(TREE, &["8"], &[Value::I64(8)], 8);
    // A left spine is unbalanced as soon as depth reaches three.
    let skew = "sum T(N(i,T,T),L)\n1=c(b(a))\n1:T=a<=0?L:N(a,b(a-1),L)\n(T)=?a(N(_,l,r):(x=c(l);y=c(r);x<0?-1:y<0?-1:!abs(x-y)>1?-1:1+!max(x,y)),L:0)\n";
    agrees(skew, &["1"], &[Value::I64(1)], 1);
    agrees(skew, &["3"], &[Value::I64(3)], -1);
}

#[test]
fn arms_stay_lazy_natively() {
    // The unselected arm divides by zero; selecting the other must not trap.
    let lazy = "sum T(N(i),L)\n=b(L)\n(T)=?a(N(v):v/0,L:5)\n";
    agrees(lazy, &[], &[], 5);
    // Selecting the payload arm does trap, proving the arm really runs.
    let traps = "sum T(N(i),L)\n=b(N(1))\n(T)=?a(N(v):v/0,L:5)\n";
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = native(traps, &[], optimization);
        assert!(!out.status.success(), "{optimization:?}");
        assert!(
            out.stderr.starts_with(b"E009"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn aliasing_a_node_never_changes_an_older_value() {
    // `b` is shared by two parents and also read directly. Values are
    // immutable, so every observation must agree.
    let shared = "sum L(C(i,L),E)\n\
                  =d(C(1,E))\n\
                  (L)=?a(C(h,_):h,E:-1)\n\
                  (L)=?a(C(_,t):?t(C(h,_):h,E:0),E:-1)\n\
                  (L)=b(a)*100+c(C(9,a))+b(C(7,a))\n";
    // b(a)=1, c(C(9,a))=1 (second element of the new node is the old head),
    // b(C(7,a))=7 — and `a` is unchanged throughout.
    agrees(shared, &[], &[], 108);
}

#[test]
fn a_million_node_list_is_torn_down_without_overflowing_the_stack() {
    // ADR 050 requires that a 10^6-node list not overflow the stack. The list
    // is built with a loop, because a recursive builder exhausts the call
    // depth guard first and reports E008 long before teardown.
    let iterative =
        "sum L(C(i,L),E)\n1=c(b(a))\n1:L=@(E,a;b>0;C(b,a),b-1;a)\n(L)=?a(C(h,_):h,E:-1)\n";
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = native(iterative, &["1000000"], optimization);
        assert!(
            out.status.success(),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
    }
}

#[test]
fn unreclaimed_nodes_report_the_ordinary_allocation_guard() {
    // Published limit: nodes are not reclaimed during execution, so a loop
    // that discards them still accumulates until the allocation guard fires.
    // That must be a clean E013, never a crash.
    let churn = "sum L(C(i,L),E)\n1=b(a)\n1=@(0,a;b>0;(x=C(b,E);?x(C(h,_):h,E:0)),b-1;a)\n";
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = native(churn, &["100000000"], optimization);
        assert!(!out.status.success(), "{optimization:?}");
        assert!(
            out.stderr.starts_with(b"E013"),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn a_sum_cannot_cross_the_native_entry_boundary() {
    let p =
        compile_with_profile("sum T(P(i),Q)\n(T)=?a(P(v):v,Q:0)\n", SourceProfile::ExprV5).unwrap();
    let error = nil_llvm::entry_runtime(&p.hir, Options::default()).unwrap_err();
    assert_eq!(error.code, "E010");
}

#[test]
fn an_unsupported_native_payload_is_refused_before_emission() {
    // Bytes payloads need the child-ownership contract B3 supplies.
    let p = compile_with_profile(
        "sum T(P(s),Q)\n=b(Q)\n(T)=?a(P(v):#v,Q:0)\n",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let error = nil_llvm::entry_runtime(&p.hir, Options::default()).unwrap_err();
    assert_eq!(error.code, "E010");
    assert!(error.message.contains("payload"), "{}", error.message);
}
