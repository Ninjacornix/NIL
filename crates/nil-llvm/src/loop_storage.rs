//! Prove that a loop-carried replacement chain can commit sparse writes at the
//! backedge. This analysis is independent of source spelling and program names.
use nil_hir::{Operation, Region, Type, ValueId};

#[derive(Debug)]
pub(crate) struct Plan {
    pub state: usize,
    /// Body instruction positions in evaluation/commit order.
    pub nodes: std::collections::BTreeMap<usize, Node>,
}

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Replace,
    Branch {
        then_nodes: std::collections::BTreeMap<usize, Node>,
        else_nodes: std::collections::BTreeMap<usize, Node>,
    },
}

// An If captures preceding values; a Loop captures only its initial operands.
// Loop regions have a new scope, so their IDs must not be treated as captures.
fn uses(operation: &Operation, target: ValueId, available: usize) -> usize {
    let count = |ids: &[ValueId]| ids.iter().filter(|id| **id == target).count();
    match operation {
        Operation::Constant(_) | Operation::Boolean(_) | Operation::Bytes(_) => 0,
        Operation::Array(ids) => count(ids),
        Operation::Repeat { value, .. } | Operation::Length(value) => usize::from(*value == target),
        Operation::Index { array, index } => count(&[*array, *index]),
        Operation::Replace {
            array,
            index,
            value,
        } => count(&[*array, *index, *value]),
        Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
            count(&[*lhs, *rhs])
        }
        Operation::Call { arguments, .. } | Operation::Intrinsic { arguments, .. } => {
            count(arguments)
        }
        Operation::Loop { initial, .. } => count(initial),
        Operation::If {
            condition,
            then_region,
            else_region,
        } => {
            usize::from(*condition == target)
                + if target.0 < available {
                    region_uses(then_region, target, available)
                        + region_uses(else_region, target, available)
                } else {
                    0
                }
        }
    }
}
fn region_uses(region: &Region, target: ValueId, inputs: usize) -> usize {
    region.results.iter().filter(|id| **id == target).count()
        + region
            .instructions
            .iter()
            .enumerate()
            .map(|(i, inst)| uses(&inst.operation, target, inputs + i))
            .sum::<usize>()
}

fn update_nodes(
    region: &Region,
    mut value: ValueId,
    root: ValueId,
    inputs: usize,
) -> Option<std::collections::BTreeMap<usize, Node>> {
    let mut nodes = std::collections::BTreeMap::new();
    // Iterate through straight chains; recurse only into validator-depth-limited
    // If regions. Long externally supplied replacement chains cannot exhaust stack.
    while value.0 >= inputs {
        if region_uses(region, value, inputs) != 1 {
            return None;
        }
        let i = value.0 - inputs;
        match &region.instructions[i].operation {
            Operation::Replace { array, .. } => {
                nodes.insert(i, Node::Replace);
                value = *array;
            }
            Operation::If {
                then_region,
                else_region,
                ..
            } => {
                let then_nodes =
                    update_nodes(then_region, then_region.results[0], root, inputs + i)?;
                let else_nodes =
                    update_nodes(else_region, else_region.results[0], root, inputs + i)?;
                nodes.insert(
                    i,
                    Node::Branch {
                        then_nodes,
                        else_nodes,
                    },
                );
                return Some(nodes);
            }
            _ => return None,
        }
    }
    (value == root).then_some(nodes)
}

pub(crate) fn plans(types: &[Type], body: &Region) -> Vec<Plan> {
    types
        .iter()
        .enumerate()
        .filter_map(|(state, ty)| {
            if !matches!(ty, Type::Array(_) | Type::Buffer | Type::Bytes) {
                return None;
            }
            let nodes = update_nodes(body, body.results[state], ValueId(state), types.len())?;
            (!nodes.is_empty()).then_some(Plan { state, nodes })
        })
        .collect()
}

// Separate from the replacement proof: retain a state root across the backedge
// only when all dynamic state follows straight identity/replacement chains and
// neither condition nor body can expose the removed intermediate bindings.
fn scalar(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Constant(_)
            | Operation::Boolean(_)
            | Operation::Length(_)
            | Operation::Index { .. }
            | Operation::Binary { .. }
            | Operation::Compare { .. }
    )
}
pub(crate) fn retain_roots(
    types: &[Type],
    condition: &Region,
    body: &Region,
    plans: &[Plan],
) -> bool {
    let last = nil_hir::liveness::last_uses(&body.instructions, &body.results, types.len());
    condition
        .instructions
        .iter()
        .all(|inst| scalar(&inst.operation))
        && types.iter().enumerate().all(|(i, ty)| {
            !matches!(ty, Type::Buffer | Type::Bytes)
                || body.results[i] == ValueId(i)
                || plans.iter().any(|p| {
                    p.state == i && p.nodes.values().all(|node| matches!(node, Node::Replace))
                })
        })
        && body
            .instructions
            .iter()
            .enumerate()
            .all(|(pos, inst)| match inst.operation {
                Operation::Replace { array, .. } => {
                    last[array.0] == Some(pos) && plans.iter().any(|p| p.nodes.contains_key(&pos))
                }
                _ => scalar(&inst.operation),
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inst(operation: Operation, ty: Type) -> nil_hir::Instruction {
        nil_hir::Instruction {
            operation,
            ty,
            span: None,
        }
    }
    fn body() -> Region {
        Region {
            instructions: vec![
                inst(Operation::Constant(0), Type::I64),
                inst(Operation::Constant(9), Type::I64),
                inst(
                    Operation::Replace {
                        array: ValueId(0),
                        index: ValueId(1),
                        value: ValueId(2),
                    },
                    Type::Array(2),
                ),
            ],
            results: vec![ValueId(3)],
        }
    }
    #[test]
    fn retained_roots_require_straight_last_use_chains() {
        let types = [Type::Buffer];
        let condition = Region {
            instructions: vec![inst(Operation::Boolean(true), Type::Bool)],
            results: vec![ValueId(1)],
        };
        let mut b = body();
        b.instructions[2].ty = Type::Buffer;
        assert!(retain_roots(&types, &condition, &b, &plans(&types, &b)));
        b.instructions.push(inst(
            Operation::Index {
                array: ValueId(0),
                index: ValueId(1),
            },
            Type::I64,
        ));
        assert!(!retain_roots(&types, &condition, &b, &plans(&types, &b)));
        b.instructions.pop();
        b.instructions.push(inst(
            Operation::Call {
                function: nil_hir::FunctionId(0),
                arguments: vec![ValueId(3)],
            },
            Type::I64,
        ));
        assert!(!retain_roots(&types, &condition, &b, &plans(&types, &b)));
    }
    #[test]
    fn only_single_use_chains_are_deferred() {
        let types = [Type::Array(2)];
        let mut b = body();
        assert_eq!(
            plans(&types, &b)[0]
                .nodes
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![2]
        );
        b.instructions.push(inst(
            Operation::Index {
                array: ValueId(3),
                index: ValueId(1),
            },
            Type::I64,
        ));
        assert!(plans(&types, &b).is_empty());
        b.instructions.pop();
        b.instructions.push(inst(
            Operation::Replace {
                array: ValueId(3),
                index: ValueId(1),
                value: ValueId(2),
            },
            Type::Array(2),
        ));
        b.results = vec![ValueId(4)];
        assert_eq!(
            plans(&types, &b)[0]
                .nodes
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }
    #[test]
    fn nested_captures_and_state_aliases_force_fallback() {
        let mut b = body();
        b.instructions
            .push(inst(Operation::Boolean(true), Type::Bool));
        b.instructions.push(inst(
            Operation::If {
                condition: ValueId(4),
                then_region: Region {
                    instructions: vec![inst(Operation::Length(ValueId(3)), Type::I64)],
                    results: vec![ValueId(5)],
                },
                else_region: Region {
                    instructions: vec![],
                    results: vec![ValueId(1)],
                },
            },
            Type::I64,
        ));
        assert!(plans(&[Type::Array(2)], &b).is_empty());
        let mut b = body();
        b.results = vec![ValueId(3), ValueId(3)];
        // The type list here changes the ID layout; count behavior is tested directly.
        assert_eq!(region_uses(&b, ValueId(3), 1), 2);
        let nested = Operation::Loop {
            initial: vec![ValueId(3)],
            condition: Region {
                instructions: vec![],
                results: vec![],
            },
            body: Region {
                instructions: vec![],
                results: vec![],
            },
            finish: Region {
                instructions: vec![],
                results: vec![],
            },
        };
        assert_eq!(uses(&nested, ValueId(3), 4), 1);
    }
}

#[cfg(test)]
mod branch_tests {
    use super::*;
    #[test]
    fn guarded_updates_and_identity_branches_can_share_a_private_state_buffer() {
        let inst = |operation, ty| nil_hir::Instruction {
            operation,
            ty,
            span: None,
        };
        let branch = Region {
            instructions: vec![
                inst(Operation::Constant(0), Type::I64),
                inst(Operation::Constant(9), Type::I64),
                inst(
                    Operation::Replace {
                        array: ValueId(0),
                        index: ValueId(2),
                        value: ValueId(3),
                    },
                    Type::Array(2),
                ),
            ],
            results: vec![ValueId(4)],
        };
        let body = Region {
            instructions: vec![inst(
                Operation::If {
                    condition: ValueId(1),
                    then_region: branch,
                    else_region: Region {
                        instructions: vec![],
                        results: vec![ValueId(0)],
                    },
                },
                Type::Array(2),
            )],
            results: vec![ValueId(2), ValueId(1)],
        };
        let p = plans(&[Type::Array(2), Type::Bool], &body);
        assert_eq!(p.len(), 1);
        let Node::Branch {
            then_nodes,
            else_nodes,
        } = &p[0].nodes[&0]
        else {
            panic!("branch proof missing")
        };
        assert!(matches!(then_nodes.get(&2), Some(Node::Replace)));
        assert!(else_nodes.is_empty());
    }
}
