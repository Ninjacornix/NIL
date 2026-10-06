//! Restricted induction proof; caller separately proves stable input storage.
use nil_hir::{BinaryOp, CompareOp, Instruction, Operation, Region, ValueId};

/// State (sequence, index) pairs whose original sequence is identity-carried,
/// condition is exactly index < length(sequence), start is a nonnegative constant,
/// and next index is index + 1. No other expression is assumed equivalent.
pub(crate) fn plans(
    initial: &[ValueId],
    condition: &Region,
    body: &Region,
    prefix: &[Instruction],
    inputs: usize,
) -> Vec<(usize, usize)> {
    let n = initial.len();
    let [length, comparison] = condition.instructions.as_slice() else {
        return vec![];
    };
    let Operation::Length(ValueId(sequence)) = length.operation else {
        return vec![];
    };
    let Operation::Compare {
        op: CompareOp::Lt,
        lhs: ValueId(index),
        rhs: ValueId(rhs),
    } = comparison.operation
    else {
        return vec![];
    };
    if sequence >= n
        || index >= n
        || rhs != n
        || condition.results != [ValueId(n + 1)]
        || body.results[sequence] != ValueId(sequence)
    {
        return vec![];
    }
    let Some(start) = initial[index]
        .0
        .checked_sub(inputs)
        .and_then(|i| prefix.get(i))
    else {
        return vec![];
    };
    if !matches!(start.operation, Operation::Constant(value) if value >= 0) {
        return vec![];
    }
    let Some(next) = body.results[index]
        .0
        .checked_sub(n)
        .and_then(|i| body.instructions.get(i))
    else {
        return vec![];
    };
    let Operation::Binary {
        op: BinaryOp::Add,
        lhs,
        rhs,
    } = next.operation
    else {
        return vec![];
    };
    if lhs != ValueId(index) {
        return vec![];
    }
    let one = rhs.0.checked_sub(n).and_then(|i| body.instructions.get(i));
    if !one.is_some_and(|i| i.operation == Operation::Constant(1)) {
        return vec![];
    }
    vec![(sequence, index)]
}
