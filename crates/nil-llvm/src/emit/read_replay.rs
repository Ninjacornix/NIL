//! Nontrapping speculative scalar evaluation from validated HIR. Failed guards
//! produce invalid values, never diagnostics; ordinary execution is the fallback.
use super::{Builder, Operand, ty};
use nil_hir::{BinaryOp, CompareOp, Instruction, Operation, Region, Type, ValueId};

#[derive(Clone)]
pub(super) struct Probe {
    pub value: Operand,
    pub valid: Operand,
}

impl Builder<'_> {
    pub(super) fn replay_invalid(&self, kind: Type) -> Probe {
        Probe {
            value: Operand {
                ty: kind,
                text: match kind {
                    Type::Bool => "false",
                    Type::I64 => "0",
                    _ => "zeroinitializer",
                }
                .into(),
            },
            valid: Operand {
                ty: Type::Bool,
                text: "false".into(),
            },
        }
    }
    pub(super) fn replay_known(value: Operand) -> Probe {
        Probe {
            value,
            valid: Operand {
                ty: Type::Bool,
                text: "true".into(),
            },
        }
    }
    pub(super) fn replay_and(&mut self, a: &Operand, b: &Operand) -> Operand {
        self.value(Type::Bool, format!("and i1 {}, {}", a.text, b.text))
    }
    fn replay_guarded(
        &mut self,
        valid: Operand,
        kind: Type,
        emit: impl FnOnce(&mut Self) -> Operand,
    ) -> Probe {
        let yes = self.block();
        let no = self.block();
        let join = self.block();
        self.conditional(&valid.text, yes, no);
        self.current = yes;
        let value = emit(self);
        let yes_end = self.current;
        self.branch(join);
        self.current = no;
        let invalid = self.replay_invalid(kind);
        self.branch(join);
        self.current = join;
        let value = self.value(
            kind,
            format!(
                "phi {} [ {}, %b{yes_end} ], [ {}, %b{no} ]",
                ty(kind),
                value.text,
                invalid.value.text
            ),
        );
        Probe { value, valid }
    }
    fn replay_region(
        &mut self,
        region: &Region,
        inputs: &[Probe],
        depth: usize,
        budget: &mut usize,
    ) -> Probe {
        let mut values = inputs.to_vec();
        let mut valid = Operand {
            ty: Type::Bool,
            text: "true".into(),
        };
        for instruction in &region.instructions {
            let value = self.replay_instruction(instruction, &values, depth, budget);
            valid = self.replay_and(&valid, &value.valid);
            values.push(value);
        }
        let mut result = values[region.results[0].0].clone();
        result.valid = self.replay_and(&valid, &result.valid);
        result
    }
    pub(super) fn replay_instruction(
        &mut self,
        instruction: &Instruction,
        inputs: &[Probe],
        depth: usize,
        budget: &mut usize,
    ) -> Probe {
        if *budget == 0 || depth == 0 {
            return self.replay_invalid(instruction.ty);
        }
        *budget -= 1;
        let get = |id: ValueId| inputs[id.0].clone();
        match &instruction.operation {
            Operation::Constant(v) => Self::replay_known(Operand {
                ty: Type::I64,
                text: v.to_string(),
            }),
            Operation::Boolean(v) => Self::replay_known(Operand {
                ty: Type::Bool,
                text: v.to_string(),
            }),
            Operation::Binary {
                op: op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul),
                lhs,
                rhs,
            } if instruction.ty == Type::I64 => {
                let a = get(*lhs);
                let b = get(*rhs);
                let valid = self.replay_and(&a.valid, &b.valid);
                let op = match op {
                    BinaryOp::Add => "add",
                    BinaryOp::Sub => "sub",
                    _ => "mul",
                };
                let value = self.value(
                    Type::I64,
                    format!("{op} i64 {}, {}", a.value.text, b.value.text),
                );
                Probe { value, valid }
            }
            Operation::Compare { op, lhs, rhs }
                if matches!(get(*lhs).value.ty, Type::I64 | Type::Bool) =>
            {
                let a = get(*lhs);
                let b = get(*rhs);
                let valid = self.replay_and(&a.valid, &b.valid);
                let predicate = match (op, a.value.ty == Type::Bool) {
                    (CompareOp::Eq, _) => "eq",
                    (CompareOp::Ne, _) => "ne",
                    (CompareOp::Lt, false) => "slt",
                    (CompareOp::Le, false) => "sle",
                    (CompareOp::Gt, false) => "sgt",
                    (CompareOp::Ge, false) => "sge",
                    (CompareOp::Lt, true) => "ult",
                    (CompareOp::Le, true) => "ule",
                    (CompareOp::Gt, true) => "ugt",
                    (CompareOp::Ge, true) => "uge",
                };
                let value = self.value(
                    Type::Bool,
                    format!(
                        "icmp {predicate} {} {}, {}",
                        ty(a.value.ty),
                        a.value.text,
                        b.value.text
                    ),
                );
                Probe { value, valid }
            }
            Operation::Length(id) if matches!(get(*id).value.ty, Type::Buffer | Type::Bytes) => {
                let sequence = get(*id);
                self.replay_guarded(sequence.valid, Type::I64, |builder| {
                    builder.dynamic_length(&sequence.value)
                })
            }
            Operation::Index { array, index }
                if matches!(get(*array).value.ty, Type::Buffer | Type::Bytes) =>
            {
                let sequence = get(*array);
                let index = get(*index);
                let valid = self.replay_and(&sequence.valid, &index.valid);
                let length = self.replay_guarded(valid.clone(), Type::I64, |builder| {
                    builder.dynamic_length(&sequence.value)
                });
                let bounds = self.value(
                    Type::Bool,
                    format!("icmp ult i64 {}, {}", index.value.text, length.value.text),
                );
                let valid = self.replay_and(&valid, &bounds);
                self.replay_guarded(valid, Type::I64, |builder| {
                    builder.value(
                        Type::I64,
                        format!(
                            "call i64 @nil_get(ptr {}, i64 {}, i64 -1, i64 -1)",
                            sequence.value.text, index.value.text
                        ),
                    )
                })
            }
            Operation::Field { record, field } => {
                let record = get(*record);
                let value = self.value(
                    instruction.ty,
                    format!(
                        "extractvalue {} {}, {field}",
                        ty(record.value.ty),
                        record.value.text
                    ),
                );
                Probe {
                    value,
                    valid: record.valid,
                }
            }
            Operation::If {
                condition,
                then_region,
                else_region,
            } if matches!(instruction.ty, Type::I64 | Type::Bool) => {
                let condition = get(*condition);
                let yes = self.block();
                let no = self.block();
                let select = self.block();
                let invalid = self.block();
                let join = self.block();
                self.conditional(&condition.valid.text, select, invalid);
                self.current = select;
                self.conditional(&condition.value.text, yes, no);
                self.current = yes;
                let left = self.replay_region(then_region, inputs, depth, budget);
                let left_end = self.current;
                self.branch(join);
                self.current = no;
                let right = self.replay_region(else_region, inputs, depth, budget);
                let right_end = self.current;
                self.branch(join);
                self.current = invalid;
                let fallback = self.replay_invalid(instruction.ty);
                self.branch(join);
                self.current = join;
                let value = self.value(
                    instruction.ty,
                    format!(
                        "phi {} [ {}, %b{left_end} ], [ {}, %b{right_end} ], [ {}, %b{invalid} ]",
                        ty(instruction.ty),
                        left.value.text,
                        right.value.text,
                        fallback.value.text
                    ),
                );
                let selected = self.value(
                    Type::Bool,
                    format!(
                        "phi i1 [ {}, %b{left_end} ], [ {}, %b{right_end} ], [ false, %b{invalid} ]",
                        left.valid.text, right.valid.text
                    ),
                );
                let valid = self.replay_and(&condition.valid, &selected);
                Probe { value, valid }
            }
            Operation::Call {
                function,
                arguments,
            } if matches!(instruction.ty, Type::I64 | Type::Bool)
                && self
                    .summaries
                    .is_some_and(|proof| proof.function(function.0)) =>
            {
                let function = &self.functions[function.0];
                let instructions = function.instructions.clone();
                let result = function.result;
                let mut values = arguments.iter().map(|id| get(*id)).collect::<Vec<_>>();
                let mut valid = Operand {
                    ty: Type::Bool,
                    text: "true".into(),
                };
                for instruction in &instructions {
                    let value = self.replay_instruction(instruction, &values, depth - 1, budget);
                    valid = self.replay_and(&valid, &value.valid);
                    values.push(value);
                }
                let mut result = values[result.0].clone();
                result.valid = self.replay_and(&valid, &result.valid);
                result
            }
            _ => self.replay_invalid(instruction.ty),
        }
    }
}
