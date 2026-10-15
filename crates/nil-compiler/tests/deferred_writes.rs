//! ADR 055 addendum D step 4a: the reference evaluator's deferred slot writes.
//!
//! The option is **off** by default, so every existing test is untouched; these
//! turn it on explicitly. In the reference the copy itself is unobservable —
//! values are immutable — so the only behaviour the option changes is the
//! **fault order**: read-phase faults strictly precede write-phase faults.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;

fn run(source: &str, defer: bool) -> Result<Value, nil_hir::Diagnostic> {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    execute_values(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            defer_slot_writes: defer,
            ..Limits::default()
        },
    )
}

/// The callee is non-leaf and returns the element being updated; 0 means it saw
/// the pre-update element.
const CALLEE: &str = "(v,i)=@(a,b,0;c<1;a,b,c+1;a[b])\n";

#[test]
fn a_sibling_call_sees_the_pre_update_element_in_both_slot_orders() {
    let read_after =
        format!("=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:b+100],b+1,c*1000+c(a,b);c)\n{CALLEE}");
    let read_before =
        format!("=b(!buffer(8,0))\n(v)=@(0,a,0;c<4;a*1000+c(b,c),b[c:c+100],c+1;a)\n{CALLEE}");
    for source in [&read_after, &read_before] {
        for defer in [false, true] {
            assert_eq!(
                run(source, defer).unwrap(),
                Value::I64(0),
                "defer={defer}: a sibling call must observe the pre-update element"
            );
        }
    }
}

#[test]
fn a_write_trap_yields_to_a_later_read_trap_only_when_deferred() {
    // Slot 1's write index and slot 3's read index are both out of range.
    let source = "=b(!buffer(8,0))\n(v)=@(a,0,0;b<1;a[100:1],b+1,c+a[200];c)\n";
    let eager = run(source, false).unwrap_err();
    let deferred = run(source, true).unwrap_err();
    assert_eq!(eager.code, "E012");
    assert_eq!(deferred.code, "E012");
    // Same diagnostic code, different site: the read-phase fault wins when the
    // write is deferred.
    assert_ne!(
        eager.span, deferred.span,
        "deferring must move the reported fault from the write to the later read"
    );
}

#[test]
fn an_update_nested_in_another_expression_is_not_deferred() {
    // The slot's result is a call *around* the update, so scope rule 1 excludes
    // it and the write still faults first.
    let source = "=#b(!buffer(8,0))\n(v):v=@(a,0,0;b<1;!slice(a[100:1],0,1),b+1,c+a[200];a)\n";
    let eager = run(source, false).unwrap_err();
    let deferred = run(source, true).unwrap_err();
    assert_eq!(eager.code, "E012");
    assert_eq!(
        eager.span, deferred.span,
        "a nested update must not be deferred"
    );
}

#[test]
fn two_slots_updating_one_buffer_keep_todays_behaviour() {
    // Both slots rewrite the same carried buffer, so neither is deferred;
    // disjointness is not attempted.
    let source = "=#b(!buffer(8,0))\n(v):v=@(a,a,0;c<1;a[100:1],b[0:1],c+1;a)\n";
    let eager = run(source, false).unwrap_err();
    let deferred = run(source, true).unwrap_err();
    assert_eq!(eager.code, "E012");
    assert_eq!(
        eager.span, deferred.span,
        "a buffer updated by two slots must not be deferred"
    );
}

#[test]
fn the_option_is_off_by_default() {
    assert!(!Limits::default().defer_slot_writes);
}
