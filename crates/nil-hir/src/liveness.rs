//! Conservative SSA liveness at instruction boundaries. Branch captures are
//! unioned; loop regions introduce independent scopes and capture only initial.
//! This describes semantic roots, not physical allocation ownership or mutability.
use crate::{Instruction, Operation, Region, ValueId};

fn operand(id: ValueId, limit: usize, live: &mut [Option<usize>], point: usize) {
    if id.0 < limit {
        live[id.0] = Some(point);
    }
}
fn captures(region: &Region, limit: usize, live: &mut [Option<usize>], point: usize) {
    for id in &region.results {
        operand(*id, limit, live, point);
    }
    for instruction in &region.instructions {
        operands(&instruction.operation, limit, live, point);
    }
}
fn operands(operation: &Operation, limit: usize, live: &mut [Option<usize>], point: usize) {
    match operation {
        Operation::Constant(_) | Operation::Boolean(_) | Operation::Bytes(_) => {}
        Operation::Array(ids)
        | Operation::Call { arguments: ids, .. }
        | Operation::Intrinsic { arguments: ids, .. }
        | Operation::Loop { initial: ids, .. } => {
            for id in ids {
                operand(*id, limit, live, point);
            }
        }
        Operation::Repeat { value, .. } | Operation::Length(value) => {
            operand(*value, limit, live, point)
        }
        Operation::Index { array, index } => {
            operand(*array, limit, live, point);
            operand(*index, limit, live, point);
        }
        Operation::Replace {
            array,
            index,
            value,
        } => {
            operand(*array, limit, live, point);
            operand(*index, limit, live, point);
            operand(*value, limit, live, point);
        }
        Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
            operand(*lhs, limit, live, point);
            operand(*rhs, limit, live, point);
        }
        Operation::If {
            condition,
            then_region,
            else_region,
        } => {
            operand(*condition, limit, live, point);
            captures(then_region, limit, live, point);
            captures(else_region, limit, live, point);
        }
    }
}

/// Final consumer instruction for every SSA input/local value. Region results
/// consume at the final boundary. Branch captures are conservatively unioned.
/// Linear-size output; no quadratic per-instruction bitmap table. Requires valid IDs.
pub fn last_uses(
    instructions: &[Instruction],
    results: &[ValueId],
    inputs: usize,
) -> Vec<Option<usize>> {
    let mut live = vec![None; inputs + instructions.len()];
    for (i, instruction) in instructions.iter().enumerate() {
        operands(&instruction.operation, inputs + i, &mut live, i);
    }
    for id in results {
        live[id.0] = Some(instructions.len());
    }
    live
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Type;
    fn inst(operation: Operation, ty: Type) -> Instruction {
        Instruction {
            operation,
            ty,
            span: None,
        }
    }
    #[test]
    fn old_reads_keep_sequence_live_until_their_last_use() {
        let instructions = vec![
            inst(Operation::Constant(0), Type::I64),
            inst(Operation::Constant(7), Type::I64),
            inst(
                Operation::Replace {
                    array: ValueId(0),
                    index: ValueId(1),
                    value: ValueId(2),
                },
                Type::Bytes,
            ),
            inst(
                Operation::Index {
                    array: ValueId(0),
                    index: ValueId(1),
                },
                Type::I64,
            ),
        ];
        let live = last_uses(&instructions, &[ValueId(3)], 1);
        assert_eq!(live[0], Some(3));
        assert_eq!(live[3], Some(4));
        assert_eq!(live[2], Some(2));
    }
    #[test]
    fn branch_capture_ids_and_loop_state_ids_have_distinct_scopes() {
        let branch = Region {
            instructions: vec![inst(Operation::Length(ValueId(0)), Type::I64)],
            results: vec![ValueId(2)],
        };
        let instructions = vec![inst(
            Operation::If {
                condition: ValueId(1),
                then_region: branch,
                else_region: Region {
                    instructions: vec![],
                    results: vec![ValueId(1)],
                },
            },
            Type::I64,
        )];
        assert_eq!(last_uses(&instructions, &[ValueId(2)], 2)[0], Some(0));
        let region = Region {
            instructions: vec![inst(Operation::Length(ValueId(0)), Type::I64)],
            results: vec![ValueId(1)],
        };
        let instructions = vec![inst(
            Operation::Loop {
                initial: vec![ValueId(1)],
                condition: region.clone(),
                body: region.clone(),
                finish: region,
            },
            Type::I64,
        )];
        let live = last_uses(&instructions, &[ValueId(2)], 2);
        assert_eq!(live[0], None);
        assert_eq!(live[1], Some(0));
    }
}
