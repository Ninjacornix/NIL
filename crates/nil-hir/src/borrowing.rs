//! Allocation-free borrowing over validated HIR, independent of LLVM inlining.
//! This is not a nontrapping/pure LLVM attribute: traps and budget checks still run.
use crate::{Instruction, Intrinsic, Operation, Region, Type, ValidatedProgram};

#[derive(Debug)]
pub struct Summaries {
    borrowing: Vec<bool>,
}
impl Summaries {
    /// Greatest fixed point: start with all functions and remove any function
    /// containing a disallowed operation or calling a removed candidate. Thus a
    /// recursive group survives only if every reachable operation is safe.
    pub fn analyze(program: &ValidatedProgram) -> Self {
        let functions = &program.program().functions;
        let mut result = Self {
            borrowing: vec![true; functions.len()],
        };
        loop {
            let mut changed = false;
            for (i, function) in functions.iter().enumerate() {
                if result.borrowing[i]
                    && result
                        .instructions(&function.instructions, &function.parameters)
                        .is_none()
                {
                    result.borrowing[i] = false;
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }
    pub fn function(&self, index: usize) -> bool {
        self.borrowing.get(index).copied().unwrap_or(false)
    }
    pub fn instruction(&self, instruction: &Instruction, inputs: &[Type]) -> bool {
        match &instruction.operation {
            Operation::Constant(_)
            | Operation::Unsigned { .. }
            | Operation::Float(_)
            | Operation::Boolean(_)
            | Operation::Length(_)
            | Operation::Index { .. }
            | Operation::Binary { .. }
            | Operation::Compare { .. } => true,
            Operation::Intrinsic { op, .. } if op.is_numeric() => true,
            Operation::Intrinsic {
                op:
                    Intrinsic::Parse
                    | Intrinsic::Equal
                    | Intrinsic::Find
                    | Intrinsic::Has
                    | Intrinsic::Size,
                ..
            } => true,
            Operation::Intrinsic {
                op: Intrinsic::Get, ..
            } => instruction.ty == Type::I64,
            Operation::Call { function, .. } => self.function(function.0),
            Operation::If {
                then_region,
                else_region,
                ..
            } => self.region(then_region, inputs) && self.region(else_region, inputs),
            Operation::Loop {
                initial,
                condition,
                body,
                finish,
            } => {
                let state = initial.iter().map(|id| inputs[id.0]).collect::<Vec<_>>();
                self.region(condition, &state)
                    && self.instructions(&body.instructions, &state).is_some()
                    && self.region(finish, &state)
            }
            _ => false,
        }
    }
    pub fn region(&self, region: &Region, inputs: &[Type]) -> bool {
        self.instructions(&region.instructions, inputs).is_some()
    }
    fn instructions(&self, instructions: &[Instruction], inputs: &[Type]) -> Option<Vec<Type>> {
        let mut types = inputs.to_vec();
        for inst in instructions {
            if !self.instruction(inst, &types) {
                return None;
            }
            types.push(inst.ty);
        }
        Some(types)
    }
}
