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
        Operation::Constant(_)
        | Operation::Unsigned { .. }
        | Operation::Float(_)
        | Operation::Boolean(_)
        | Operation::Bytes(_) => {}
        Operation::RecordMap(_) => {}
        Operation::Field { record, .. } => operand(*record, limit, live, point),
        Operation::UpdateField { record, value, .. } => {
            operand(*record, limit, live, point);
            operand(*value, limit, live, point);
        }
        Operation::Record { fields: ids, .. }
        | Operation::Array(ids)
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

/// A scalar instruction may borrow live sequence operands without introducing
/// region roots: it cannot allocate/collect/reallocate or return a sequence.
/// Trapping arithmetic/indexing is permitted; this proof never permits eager
/// evaluation of lazy arms. Calls, intrinsics and nested loops remain unproved.
pub fn rootless_scalar_instruction(instruction: &Instruction, inputs: &[crate::Type]) -> bool {
    use crate::Type;
    matches!(
        instruction.ty,
        Type::I64 | Type::U64 | Type::U128 | Type::F64 | Type::Bool
    ) && match &instruction.operation {
        Operation::Constant(_)
        | Operation::Unsigned { .. }
        | Operation::Float(_)
        | Operation::Boolean(_)
        | Operation::Length(_)
        | Operation::Index { .. }
        | Operation::Binary { .. }
        | Operation::Compare { .. } => true,
        Operation::If {
            then_region,
            else_region,
            ..
        } => {
            rootless_scalar_region(then_region, inputs)
                && rootless_scalar_region(else_region, inputs)
        }
        _ => false,
    }
}

/// Recursively prove nonallocating scalar lazy regions. Captures remain part of
/// semantic last-use analysis; only their redundant physical roots are omitted.
pub fn rootless_scalar_region(region: &Region, inputs: &[crate::Type]) -> bool {
    let mut types = inputs.to_vec();
    for instruction in &region.instructions {
        if !rootless_scalar_instruction(instruction, &types) {
            return false;
        }
        types.push(instruction.ty);
    }
    region
        .results
        .iter()
        .all(|id| matches!(types[id.0], crate::Type::I64 | crate::Type::Bool))
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
    fn scalar_lazy_proof_preserves_captures_and_rejects_escape_or_allocation() {
        let inputs = [Type::Bytes, Type::Bool];
        let read = Region {
            instructions: vec![inst(Operation::Length(ValueId(0)), Type::I64)],
            results: vec![ValueId(2)],
        };
        let nested = Region {
            instructions: vec![inst(
                Operation::If {
                    condition: ValueId(1),
                    then_region: read.clone(),
                    else_region: read.clone(),
                },
                Type::I64,
            )],
            results: vec![ValueId(2)],
        };
        assert!(rootless_scalar_region(&nested, &inputs));
        assert_eq!(
            last_uses(&nested.instructions, &nested.results, 2)[0],
            Some(0)
        );
        assert!(!rootless_scalar_region(
            &Region {
                instructions: vec![],
                results: vec![ValueId(0)]
            },
            &inputs
        ));
        for operation in [
            Operation::Bytes(vec![0]),
            Operation::Call {
                function: crate::FunctionId(0),
                arguments: vec![ValueId(0)],
            },
            Operation::Intrinsic {
                op: crate::Intrinsic::Out,
                arguments: vec![ValueId(0)],
            },
        ] {
            let region = Region {
                instructions: vec![inst(operation, Type::I64)],
                results: vec![ValueId(2)],
            };
            assert!(!rootless_scalar_region(&region, &inputs));
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
