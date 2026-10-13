//! Integer-keyed sets must agree exactly between reference and native at both
//! optimization levels (ADR 051 part 2).
//!
//! Keys are the eight big-endian bytes of the i64, formed inside the runtime,
//! so membership and insertion allocate nothing per call and the existing
//! byte-keyed table is reused. Correctness first; an integer hash is an
//! optimization to justify by measurement.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use nil_llvm::{Optimization, Options};

fn agrees(source: &str, argument: &str, values: &[Value], expected: i64) {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values(
            &program.hir,
            FunctionId(0),
            values,
            Limits {
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
                ..Limits::default()
            },
        )
        .unwrap(),
        Value::I64(expected),
        "reference: {source} {argument}"
    );
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &program.hir,
            Options {
                optimization,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            },
            &if argument.is_empty() {
                Vec::new()
            } else {
                vec![argument.to_string()]
            },
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            expected.to_string(),
            "{source} {argument} at {optimization:?}"
        );
    }
}

/// Leading element is the count; 1 means a duplicate was found.
const DUPLICATE: &str =
    "(v)=@(a,!set(),1;c<=a[0]?(!has(b,a[c])?false:true):false;a,!add(b,a[c]),c+1;c<=a[0]?1:0)\n";

#[test]
fn membership_agrees_including_the_extremes_of_the_key_range() {
    let buffer = |values: Vec<i64>| Value::Buffer(values.into());
    agrees(DUPLICATE, "[3,10,20,30]", &[buffer(vec![3, 10, 20, 30])], 0);
    agrees(DUPLICATE, "[3,10,20,10]", &[buffer(vec![3, 10, 20, 10])], 1);
    // Negatives must not alias positives through the key encoding.
    agrees(DUPLICATE, "[2,-5,5]", &[buffer(vec![2, -5, 5])], 0);
    agrees(DUPLICATE, "[2,-5,-5]", &[buffer(vec![2, -5, -5])], 1);
    // The extremes are ordinary keys.
    agrees(
        DUPLICATE,
        "[3,-9223372036854775808,9223372036854775807,-9223372036854775808]",
        &[buffer(vec![3, i64::MIN, i64::MAX, i64::MIN])],
        1,
    );
    agrees(
        DUPLICATE,
        "[3,-9223372036854775808,9223372036854775807,0]",
        &[buffer(vec![3, i64::MIN, i64::MAX, 0])],
        0,
    );
}

#[test]
fn size_and_removal_agree_and_removal_keeps_the_survivors() {
    // Build 1..n, remove the first element, then count what is left and probe.
    let source = "1=(s=@(!set(),1,a;b<=c;!add(a,b),b+1,c;a);\
                  t=!remove(s,1);\
                  !size(s)*1000+!size(t)*10+(!has(t,1)?5:0)+(!has(t,a)?1:0))\n";
    agrees(source, "4", &[Value::I64(4)], 4031);
    // Removing an absent key changes nothing, and removing twice is stable.
    let absent = "1=(s=!add(!set(),a);t=!remove(!remove(s,a),a);!size(s)*10+!size(t))\n";
    agrees(absent, "7", &[Value::I64(7)], 10);
}

#[test]
fn a_set_is_an_immutable_value_natively_too() {
    // Adding to an alias must not change the original at either level.
    let source = "=(a=!add(!set(),1);b=!add(a,2);!size(a)*10+!size(b))\n";
    agrees(source, "", &[], 12);
    let removal = "=(a=!add(!add(!set(),1),2);b=!remove(a,1);!size(a)*10+!size(b))\n";
    agrees(removal, "", &[], 21);
}

#[test]
fn bulk_construction_agrees_and_keeps_first_occurrence_order() {
    // Leading element is the count; 1 means a duplicate was found.
    const BULK: &str = "(v)=!size(!set(!slice(a,1,a[0])))!=a[0]?1:0\n";
    let buffer = |values: Vec<i64>| Value::Buffer(values.into());
    agrees(BULK, "[3,10,20,30]", &[buffer(vec![3, 10, 20, 30])], 0);
    agrees(BULK, "[3,10,20,10]", &[buffer(vec![3, 10, 20, 10])], 1);
    // An empty source is an empty set, not a fault.
    agrees(BULK, "[0]", &[buffer(vec![0])], 0);
    // The extremes are ordinary keys here too.
    agrees(
        BULK,
        "[2,-9223372036854775808,-9223372036854775808]",
        &[buffer(vec![2, i64::MIN, i64::MIN])],
        1,
    );
    agrees(
        BULK,
        "[2,9223372036854775807,-9223372036854775808]",
        &[buffer(vec![2, i64::MAX, i64::MIN])],
        0,
    );
    // Bulk and incremental construction must agree on size.
    let same = "1=(!size(!set(!buffer(a,7)))*10+!size(@(!set(),0,a;b<c;!add(a,7),b+1,c;a)))\n";
    agrees(same, "4", &[Value::I64(4)], 11);
}
