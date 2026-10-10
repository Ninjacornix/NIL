//! Bounded scalar snapshot proof for a read of the original after replacement.
//! Checks stay at the replacement; no trapping operation moves earlier.
use nil_hir::{BinaryOp, Instruction, Operation, Type, ValueId};
use std::collections::{BTreeMap, BTreeSet};

/// Scalar observations following a selected sequence-valued region. Region
/// results must be identity/replacement continuations of this exact SSA origin;
/// the runtime still checks independent physical aliases before reusing storage.
pub(crate) fn selected_continuation(
    instructions: &[Instruction],
    inputs: usize,
    position: usize,
    root: ValueId,
    last: &[Option<usize>],
) -> Option<BTreeSet<usize>> {
    let Operation::If {
        then_region,
        else_region,
        ..
    } = &instructions[position].operation
    else {
        return None;
    };
    let available = inputs + position;
    crate::loop_storage::update_nodes(then_region, then_region.results[0], root, available)?;
    crate::loop_storage::update_nodes(else_region, else_region.results[0], root, available)?;
    let end = last[root.0]?;
    if end <= position || end >= instructions.len() || end - position > 128 {
        return None;
    }
    let dependencies = (position + 1..=end)
        .map(|site| nil_hir::liveness::last_uses(&instructions[site..=site], &[], inputs + site))
        .collect::<Vec<_>>();
    let mut needed = BTreeSet::new();
    for (offset, uses) in dependencies.iter().enumerate() {
        if uses[root.0].is_some() {
            needed.insert(position + 1 + offset);
        }
    }
    let mut pending = needed.iter().copied().collect::<Vec<_>>();
    while let Some(site) = pending.pop() {
        if !matches!(instructions[site].ty, Type::I64 | Type::Bool) {
            return None;
        }
        for (id, used) in dependencies[site - position - 1].iter().enumerate() {
            if used.is_some() && id >= available {
                let producer = id - inputs;
                if producer == position {
                    return None;
                }
                if needed.insert(producer) {
                    pending.push(producer);
                }
            }
        }
    }
    Some(needed)
}

/// Backward SSA slice of every observation of `root` after `anchor`, closed over
/// scalar dependencies. `None` rejects: a sequence-typed consumer, an escape as a
/// region result, a dependency on the anchor's own result, an unsupported type or
/// an interval beyond the 128-instruction budget. Independent effects,
/// allocations and updates keep their ordinary sites.
fn scalar_slice(
    instructions: &[Instruction],
    inputs: usize,
    anchor: usize,
    root: ValueId,
    last: &[Option<usize>],
) -> Option<BTreeSet<usize>> {
    let end = last[root.0]?;
    if end <= anchor || end >= instructions.len() || end - anchor > 128 {
        return None;
    }
    // Reuse semantic liveness's operand traversal so captures and nested
    // region-local IDs cannot be confused with values in this SSA scope.
    let dependencies = (anchor + 1..=end)
        .map(|site| nil_hir::liveness::last_uses(&instructions[site..=site], &[], inputs + site))
        .collect::<Vec<_>>();
    let mut needed = BTreeSet::new();
    for (offset, uses) in dependencies.iter().enumerate() {
        if uses[root.0].is_some() {
            needed.insert(anchor + 1 + offset);
        }
    }
    let mut pending = needed.iter().copied().collect::<Vec<_>>();
    while let Some(site) = pending.pop() {
        if !matches!(instructions[site].ty, Type::I64 | Type::Bool) {
            return None;
        }
        for (id, used) in dependencies[site - anchor - 1].iter().enumerate() {
            if used.is_some() && id >= inputs + anchor {
                let producer = id - inputs;
                if producer == anchor {
                    return None;
                }
                if needed.insert(producer) {
                    pending.push(producer);
                }
            }
        }
    }
    (!needed.is_empty()).then_some(needed)
}

fn sequence_type(instructions: &[Instruction], types: &[Type], id: ValueId) -> Type {
    if id.0 < types.len() {
        types[id.0]
    } else {
        instructions[id.0 - types.len()].ty
    }
}

/// R2 caller/callee root split. At a call whose argument has no remaining
/// observation other than proved scalar snapshots, the caller may surrender its
/// root so the callee's own root is the only owner and its update can reuse the
/// allocation. Returns the call position, the argument to surrender and the
/// sites whose reads the snapshots replace.
///
/// Admission follows ADR 049's R2 addendum: one argument slot by SSA identity,
/// `owns` on the callee parameter, known return provenance, a full scalar slice,
/// and an existing caller root. At most one argument per call is transferred, so
/// two snapshot sets can never contend for the same replayed site.
pub(crate) fn call_transfers(
    instructions: &[Instruction],
    types: &[Type],
    last: &[Option<usize>],
    ownership: &nil_hir::parameters::Ownership,
) -> BTreeMap<usize, (ValueId, BTreeSet<usize>)> {
    let inputs = types.len();
    let mut found = BTreeMap::new();
    for (position, instruction) in instructions.iter().enumerate() {
        let Operation::Call {
            function,
            arguments,
        } = &instruction.operation
        else {
            continue;
        };
        // An unknown returned provenance blocks transfer even when the
        // parameter itself looks dead: returning an alias hands a live
        // obligation back, it is not evidence the parameter was fresh.
        if !ownership.origins(function.0).is_known() {
            continue;
        }
        let transfer = arguments.iter().enumerate().find_map(|(slot, id)| {
            if !matches!(
                sequence_type(instructions, types, *id),
                Type::Buffer | Type::Bytes
            ) {
                return None;
            }
            // Repeated arguments keep their separate roots, so f(a,a) never
            // transfers; compare by SSA identity rather than by storage.
            if arguments.iter().filter(|other| *other == id).count() != 1 {
                return None;
            }
            if !ownership.owns(function.0, slot) {
                return None;
            }
            scalar_slice(instructions, inputs, position, *id, last).map(|needed| (*id, needed))
        });
        if let Some(transfer) = transfer {
            found.insert(position, transfer);
        }
    }
    found
}

/// Backward SSA slice of every subsequent observation of an original sequence.
/// Independent effects/allocations/updates remain at their ordinary sites. Only
/// scalar dependencies are replayed, under nontrapping runtime guards; escaping
/// observations and dependencies on new sequences remain conservative.
/// Bounded code retains ordinary instruction/call budgets.
pub(crate) fn replay_intervals(
    instructions: &[Instruction],
    types: &[Type],
    last: &[Option<usize>],
) -> BTreeMap<usize, BTreeSet<usize>> {
    let inputs = types.len();
    let mut found = BTreeMap::new();
    let mut covered = None;
    for (position, instruction) in instructions.iter().enumerate() {
        if covered.is_some_and(|end| position <= end) {
            continue;
        }
        let Operation::Replace { array, .. } = instruction.operation else {
            continue;
        };
        if !matches!(
            sequence_type(instructions, types, array),
            Type::Buffer | Type::Bytes
        ) {
            continue;
        }
        if let Some(needed) = scalar_slice(instructions, inputs, position, array, last) {
            covered = Some(*needed.last().expect("nonempty slice"));
            found.insert(position, needed);
        }
    }
    found
}

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
        ) if aop == bop && matches!(aop, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) => {
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
/// host effect. The snapshot replaces the final subsequent use; B2 separately
/// reviews these conservative interval restrictions under ordinary memory.
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
/// left operand. Its byte checks and instruction position are retained; stack storage has no arena charge.
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
