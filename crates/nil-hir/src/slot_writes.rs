//! ADR 055 D: read-phase expressions precede top-level carried-buffer writes.
//!
//! Scheduling ordinary immutable Replace operations after the readers lets the
//! existing last-use and runtime uniqueness checks decide whether to copy. No
//! owner is surrendered early, and aliases stay rooted by ordinary liveness.
use crate::{Instruction, Operation, Region, Type, ValidatedProgram, ValueId};
use std::collections::BTreeSet;

/// Remap operands, including captured values in lazy regions. Loop regions have
/// independent input scopes and therefore do not capture their parent's IDs.
fn remap(op: &mut Operation, map: &[ValueId], first: usize) {
    let id = |v: &mut ValueId| *v = map[v.0];
    let ids = |vs: &mut Vec<ValueId>| vs.iter_mut().for_each(id);
    match op {
        Operation::Constant(_)
        | Operation::Unsigned { .. }
        | Operation::Float(_)
        | Operation::Boolean(_)
        | Operation::Bytes(_)
        | Operation::RecordMap(_) => {}
        Operation::RecordBuffer { length, fill, .. } => {
            id(length);
            id(fill);
        }
        Operation::Record { fields, .. }
        | Operation::Construct { fields, .. }
        | Operation::Array(fields)
        | Operation::Intrinsic {
            arguments: fields, ..
        }
        | Operation::Call {
            arguments: fields, ..
        }
        | Operation::PluginCall {
            arguments: fields, ..
        } => ids(fields),
        Operation::SumTag { value, .. }
        | Operation::SumField { value, .. }
        | Operation::Repeat { value, .. }
        | Operation::Length(value)
        | Operation::Unary { value, .. } => id(value),
        Operation::Field { record, .. } => id(record),
        Operation::UpdateField { record, value, .. } => {
            id(record);
            id(value);
        }
        Operation::Index { array, index } => {
            id(array);
            id(index);
        }
        Operation::Replace {
            array,
            index,
            value,
        } => {
            id(array);
            id(index);
            id(value);
        }
        Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
            id(lhs);
            id(rhs);
        }
        Operation::If {
            condition,
            then_region,
            else_region,
        } => {
            id(condition);
            for region in [then_region, else_region] {
                let mut local = map.to_vec();
                for (offset, instruction) in region.instructions.iter_mut().enumerate() {
                    remap(&mut instruction.operation, &local, first + offset);
                    local.push(ValueId(first + offset));
                }
                for result in &mut region.results {
                    *result = local[result.0];
                }
            }
        }
        Operation::Loop { initial, .. } => ids(initial),
    }
}

/// Only direct chains whose intermediate values have no other consumers can
/// move. A nested update (including one captured by a lazy arm) stays eager.
fn order(body: &Region, types: &[Type]) -> Vec<usize> {
    let inputs = types.len();
    let mut chains = Vec::new();
    for result in &body.results {
        let mut value = *result;
        let mut chain = Vec::new();
        while value.0 >= inputs {
            let position = value.0 - inputs;
            let instruction = &body.instructions[position];
            let Operation::Replace { array, .. } = instruction.operation else {
                break;
            };
            if !matches!(instruction.ty, Type::Buffer | Type::Bytes) {
                break;
            }
            chain.push(position);
            value = array;
        }
        if !chain.is_empty()
            && value.0 < inputs
            && matches!(types[value.0], Type::Buffer | Type::Bytes)
        {
            chain.reverse();
            chains.push((value, chain));
        }
    }
    let mut deferred = Vec::new();
    for (base, chain) in &chains {
        if chains.iter().filter(|(other, _)| other == base).count() != 1 {
            continue;
        }
        let mut valid = true;
        for (index, &position) in chain.iter().enumerate() {
            let value = ValueId(inputs + position);
            let next = chain.get(index + 1).copied();
            // Include lazy captures: a read in an If or call argument is still
            // an independent consumer of the intermediate updated value.
            for (consumer, instruction) in body.instructions.iter().enumerate().skip(position + 1) {
                if Some(consumer) == next {
                    continue;
                }
                if crate::liveness::uses(&instruction.operation, value) {
                    valid = false;
                    break;
                }
            }
            let result_uses = body.results.iter().filter(|id| **id == value).count();
            if result_uses != usize::from(next.is_none()) {
                valid = false;
            }
        }
        if valid {
            deferred.extend(chain);
        }
    }
    let set = deferred.iter().copied().collect::<BTreeSet<_>>();
    (0..body.instructions.len())
        .filter(|p| !set.contains(p))
        .chain(deferred)
        .collect()
}

fn schedule(body: &mut Region, types: &[Type]) {
    let order = order(body, types);
    if order.iter().copied().eq(0..body.instructions.len()) {
        return;
    }
    let inputs = types.len();
    let mut map = (0..inputs + body.instructions.len())
        .map(ValueId)
        .collect::<Vec<_>>();
    for (new, old) in order.iter().enumerate() {
        map[inputs + old] = ValueId(inputs + new);
    }
    let instructions = order
        .iter()
        .enumerate()
        .map(|(new, &old)| {
            let mut instruction = body.instructions[old].clone();
            // Only values in scope at the old instruction can be captured.
            remap(
                &mut instruction.operation,
                &map[..inputs + old],
                inputs + new,
            );
            instruction
        })
        .collect();
    body.instructions = instructions;
    for result in &mut body.results {
        *result = map[result.0];
    }
}

fn nested(instructions: &mut [Instruction], inputs: &[Type]) {
    let mut types = inputs.to_vec();
    for instruction in instructions {
        match &mut instruction.operation {
            Operation::If {
                then_region,
                else_region,
                ..
            } => {
                nested(&mut then_region.instructions, &types);
                nested(&mut else_region.instructions, &types);
            }
            Operation::Loop {
                initial,
                condition,
                body,
                finish,
            } => {
                let state = initial.iter().map(|id| types[id.0]).collect::<Vec<_>>();
                nested(&mut condition.instructions, &state);
                nested(&mut body.instructions, &state);
                nested(&mut finish.instructions, &state);
                schedule(body, &state);
            }
            _ => {}
        }
        types.push(instruction.ty);
    }
}

/// Lower the loop-slot phase rule before execution or native root analysis.
/// Flat dynamic sequences exist only in expr-v5; earlier profiles are untouched.
pub fn lower(program: &ValidatedProgram) -> ValidatedProgram {
    let mut program = program.program().clone();
    for function in &mut program.functions {
        nested(&mut function.instructions, &function.parameters);
    }
    crate::validate(program).expect("slot-write scheduling preserves SSA scopes and types")
}
