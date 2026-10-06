//! Bounded scalar snapshot proof for a read of the original after replacement.
//! Checks stay at the replacement; no trapping operation moves earlier.
use nil_hir::{BinaryOp, Instruction, Operation, Type, ValueId};
use std::collections::BTreeMap;

fn same(
    instructions: &[Instruction],
    inputs: usize,
    a: ValueId,
    b: ValueId,
    budget: usize,
) -> bool {
    if a == b {
        return true;
    }
    if budget == 0 || a.0 < inputs || b.0 < inputs {
        return false;
    }
    match (
        &instructions[a.0 - inputs].operation,
        &instructions[b.0 - inputs].operation,
    ) {
        (Operation::Constant(a), Operation::Constant(b)) => a == b,
        (
            Operation::Binary {
                op: aop,
                lhs: al,
                rhs: ar,
            },
            Operation::Binary {
                op: bop,
                lhs: bl,
                rhs: br,
            },
        ) if aop == bop && *aop != BinaryOp::Div => {
            same(instructions, inputs, *al, *bl, budget - 1)
                && same(instructions, inputs, *ar, *br, budget - 1)
        }
        _ => false,
    }
}
fn nontrapping(i: &Instruction) -> bool {
    match i.operation {
        Operation::Constant(_) | Operation::Boolean(_) => true,
        Operation::Binary {
            op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul,
            ..
        } => i.ty == Type::I64,
        Operation::Compare { .. } => true,
        _ => false,
    }
}
/// Replacement position -> later scalar read position. Between them nothing can
/// allocate, escape, trap (except unchanged instrumentation ticks), or observe a
/// host effect. Hence the original's prospective copy charge is only observable
/// at the replacement check, and the snapshot replaces its final subsequent use.
pub(crate) fn snapshots(
    instructions: &[Instruction],
    types: &[Type],
    last: &[Option<usize>],
) -> BTreeMap<usize, usize> {
    let inputs = types.len();
    let mut found = BTreeMap::new();
    for (pos, i) in instructions.iter().enumerate() {
        let Operation::Replace { array, index, .. } = i.operation else {
            continue;
        };
        let ty = if array.0 < inputs {
            types[array.0]
        } else {
            instructions[array.0 - inputs].ty
        };
        if !matches!(ty, Type::Buffer | Type::Bytes) {
            continue;
        }
        let Some(end) = last[array.0] else {
            continue;
        };
        if end <= pos || end >= instructions.len() {
            continue;
        }
        let Operation::Index {
            array: original,
            index: read_index,
        } = instructions[end].operation
        else {
            continue;
        };
        if original != array || !same(instructions, inputs, index, read_index, 32) {
            continue;
        }
        if instructions[pos + 1..end].iter().all(nontrapping) {
            found.insert(pos, end);
        }
    }
    found
}

/// Adjacent, single-consumer RHS. It cannot escape, cross a call, or alias the
/// left operand. Its ordinary quota charge and instruction position are retained.
pub(crate) fn singleton(
    instructions: &[Instruction],
    inputs: usize,
    position: usize,
    last: &[Option<usize>],
) -> bool {
    let Some(next) = instructions.get(position + 1) else {
        return false;
    };
    let Operation::Intrinsic {
        op: nil_hir::Intrinsic::Concat,
        arguments,
    } = &next.operation
    else {
        return false;
    };
    let rhs = ValueId(inputs + position);
    if arguments[1] != rhs
        || arguments[0] == rhs
        || last[rhs.0] != Some(position + 1)
        || last[arguments[0].0] != Some(position + 1)
    {
        return false;
    }
    match &instructions[position].operation {
        Operation::Bytes(bytes) => bytes.len() == 1,
        Operation::Intrinsic {
            op: nil_hir::Intrinsic::Bytes | nil_hir::Intrinsic::Buffer,
            arguments,
        } => {
            arguments[0].0 >= inputs
                && matches!(
                    instructions[arguments[0].0 - inputs].operation,
                    Operation::Constant(1)
                )
        }
        _ => false,
    }
}
