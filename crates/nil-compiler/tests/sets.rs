//! Integer-keyed sets on the reference path (ADR 051 part 2).
//!
//! These cover frontend and reference semantics. Keys are the eight big-endian
//! bytes of the i64 internally,
//! which is injective, so the whole range works including `i64::MIN`.
use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{Diagnostic, FunctionId};

fn check(source: &str) -> Result<nil_compiler::CompiledProgram, Diagnostic> {
    compile_with_profile(source, SourceProfile::ExprV5)
}
fn run(source: &str, args: &[Value]) -> Result<Value, Diagnostic> {
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

#[test]
fn membership_and_size_need_no_byte_formatting() {
    // The measured spelling: !set(), !add, !has — no !format on the key path.
    let source = "1=(s=!add(!add(!set(),a),a+1);!has(s,a)?!size(s):-1)\n";
    assert_eq!(run(source, &[Value::I64(7)]).unwrap(), Value::I64(2));
    // Re-adding an existing key is idempotent, like !put on a map.
    let again = "1=(s=!add(!add(!set(),a),a);!size(s))\n";
    assert_eq!(run(again, &[Value::I64(7)]).unwrap(), Value::I64(1));
    // Absent keys are false, not a trap.
    let absent = "1=(s=!add(!set(),a);!has(s,a+1)?1:0)\n";
    assert_eq!(run(absent, &[Value::I64(7)]).unwrap(), Value::I64(0));
}

#[test]
fn set_iteration_returns_keys_in_insertion_order() {
    let source = "=(s=!add(!add(!set(),40),10);!key(s,0)*100+!key(s,1))\n";
    assert_eq!(run(source, &[]).unwrap(), Value::I64(4010));
}

#[test]
fn the_whole_i64_range_is_a_valid_key() {
    // i64::MIN has no positive magnitude, and negatives must not alias
    // positives through the internal encoding.
    let source = "1=(s=!add(!add(!add(!set(),i64min),i64max),-1);!has(s,a)?1:0)\n";
    for key in [i64::MIN, i64::MAX, -1] {
        assert_eq!(
            run(source, &[Value::I64(key)]).unwrap(),
            Value::I64(1),
            "{key}"
        );
    }
    for key in [0, 1, -2] {
        assert_eq!(
            run(source, &[Value::I64(key)]).unwrap(),
            Value::I64(0),
            "{key}"
        );
    }
    // Distinct extremes stay distinct.
    let size = "=(s=!add(!add(!add(!set(),i64min),i64max),0);!size(s))\n";
    assert_eq!(run(size, &[]).unwrap(), Value::I64(3));
}

#[test]
fn removal_drops_only_the_named_key_and_keeps_insertion_order() {
    let source = "=(s=!remove(!add(!add(!add(!set(),10),20),30),20);\
                  !size(s)*100+(!has(s,10)?10:0)+(!has(s,20)?5:0)+(!has(s,30)?1:0))\n";
    // Two left, 10 and 30 present, 20 gone.
    assert_eq!(run(source, &[]).unwrap(), Value::I64(211));
    // Removing an absent key is not a trap and changes nothing.
    let absent = "=(s=!remove(!add(!set(),10),99);!size(s))\n";
    assert_eq!(run(absent, &[]).unwrap(), Value::I64(1));
    // Removing everything leaves an empty set.
    let empty = "=(s=!remove(!add(!set(),10),10);!size(s))\n";
    assert_eq!(run(empty, &[]).unwrap(), Value::I64(0));
}

#[test]
fn a_set_is_an_immutable_value_so_aliases_keep_their_contents() {
    // Adding to an alias must not change the original, like every other
    // sequence or map in this language.
    let source = "=(a=!add(!set(),1);b=!add(a,2);!size(a)*10+!size(b))\n";
    assert_eq!(run(source, &[]).unwrap(), Value::I64(12));
    // And removal is equally non-destructive.
    let removal = "=(a=!add(!add(!set(),1),2);b=!remove(a,1);!size(a)*10+!size(b))\n";
    assert_eq!(run(removal, &[]).unwrap(), Value::I64(21));
}

#[test]
fn shared_set_removal_charges_the_copy_before_cloning() {
    let source = "=(s=!add(!set(),1);t=!remove(s,1);!size(s)*10+!size(t))\n";
    let program = check(source).unwrap();
    let error = execute_values(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            memory_bytes: 500,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "E013");
}

#[test]
fn contains_duplicate_is_expressible_without_formatting_keys() {
    // The program the spelling was measured on, in its kernel form.
    let source = "(v)=@(a,!set(),1;c<=a[0]?(!has(b,a[c])?false:true):false;a,!add(b,a[c]),c+1;c<=a[0]?1:0)\n";
    // Leading element is the count; 1 means a duplicate exists.
    let distinct = Value::Buffer(vec![3, 10, 20, 30].into());
    let repeated = Value::Buffer(vec![3, 10, 20, 10].into());
    assert_eq!(run(source, &[distinct]).unwrap(), Value::I64(0));
    assert_eq!(run(source, &[repeated]).unwrap(), Value::I64(1));
    // Negative and extreme values behave the same, with no formatting.
    let negatives = Value::Buffer(vec![3, -5, i64::MIN, -5].into());
    assert_eq!(run(source, &[negatives]).unwrap(), Value::I64(1));
}

#[test]
fn set_operands_are_typed() {
    // A set is not a byte-keyed map: its key is an i64.
    assert_eq!(
        check("1=(s=!add(!set(),a);!has(s,!format(a))?1:0)\n")
            .unwrap_err()
            .code,
        "E007"
    );
    // And a map is not a set.
    assert_eq!(check("1=!add(!map(),a)\n").unwrap_err().code, "E007");
}
