//! The integer-keyed map (`!imap`) must agree exactly between reference and
//! native at both optimization levels (ADR 051 part 2).
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

/// Build an index map from the input, then probe it.
const INDEX: &str = "(v)=@(a,!imap(),1;c<=a[0];a,!put(b,a[c],c),c+1;\
                     !size(b)*100+!get(b,a[1])*10+(!has(b,-5)?1:0))\n";

#[test]
fn lookup_agrees_including_the_extremes_and_repeated_keys() {
    let buffer = |values: Vec<i64>| Value::Buffer(values.into());
    agrees(INDEX, "[3,7,-5,7]", &[buffer(vec![3, 7, -5, 7])], 231);
    // The extremes are ordinary keys and must not collide.
    agrees(
        INDEX,
        "[2,-9223372036854775808,9223372036854775807]",
        &[buffer(vec![2, i64::MIN, i64::MAX])],
        210,
    );
    // A repeated key keeps its position and takes the later value.
    agrees(INDEX, "[3,-5,-5,-5]", &[buffer(vec![3, -5, -5, -5])], 131);
}

#[test]
fn iteration_is_insertion_order() {
    // Keys 3, 2, 1 inserted in that order must read back 3, 2, 1.
    let order = "(v)=@(a,!imap(),1;c<=a[0];a,!put(b,a[c],c),c+1;\
                 @(b,0,0;c<!size(a);a,b*10+!key(a,c),c+1;b))\n";
    agrees(
        order,
        "[3,3,2,1]",
        &[Value::Buffer(vec![3, 3, 2, 1].into())],
        321,
    );
    // A repeat does not move the existing key to the end.
    agrees(
        order,
        "[3,1,2,1]",
        &[Value::Buffer(vec![3, 1, 2, 1].into())],
        12,
    );
}

#[test]
fn removal_keeps_order_and_a_map_is_an_immutable_value() {
    let removal = "=(m=!put(!put(!put(!imap(),1,10),2,20),3,30);\
                   n=!remove(m,2);\
                   !size(m)*1000+!size(n)*100+!key(n,1)*10+!get(n,3))\n";
    // Survivors keep their order, so key 3 moves into position 1: size 3,
    // size 2, key(n,1) = 3, get(n,3) = 30.
    agrees(removal, "", &[], 3260);
    // Removing an absent key changes nothing.
    let absent = "=(m=!put(!imap(),1,10);n=!remove(m,9);!size(m)*10+!size(n))\n";
    agrees(absent, "", &[], 11);
    // The original survives an update through an alias.
    let alias = "=(m=!put(!imap(),1,10);n=!put(m,2,20);!size(m)*10+!size(n))\n";
    agrees(alias, "", &[], 12);
}

#[test]
fn a_missing_key_is_a_fault_not_a_default() {
    let program = compile_with_profile("=!get(!imap(),7)\n", SourceProfile::ExprV5).unwrap();
    assert!(
        execute_values(
            &program.hir,
            FunctionId(0),
            &[],
            Limits {
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
                ..Limits::default()
            },
        )
        .is_err(),
        "a missing integer key must fault"
    );
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &program.hir,
            Options {
                optimization,
                memory_bytes: nil_hir::MAX_DYNAMIC_BYTES as u64,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(!out.status.success(), "{optimization:?} must fault");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("E019"),
            "{optimization:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
