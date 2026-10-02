//! Allocation-free scalar borrowing over validated HIR, independent of LLVM inlining.
//! This is not a nontrapping/pure LLVM attribute: traps and budget checks still run.
use crate::{Instruction, Intrinsic, Operation, Region, Type, ValidatedProgram};

#[derive(Debug)]
pub struct Summaries {
    scalar_borrowing: Vec<bool>,
}
impl Summaries {
    /// Greatest fixed point: start with scalar candidates and remove any function
    /// containing a disallowed operation or calling a removed candidate. Thus a
    /// recursive group survives only if every reachable operation is safe.
    pub fn analyze(program: &ValidatedProgram) -> Self {
        let functions = &program.program().functions;
        let mut result = Self {
            scalar_borrowing: functions.iter().map(|f| scalar(f.result_type)).collect(),
        };
        loop {
            let mut changed = false;
            for (i, function) in functions.iter().enumerate() {
                if result.scalar_borrowing[i]
                    && !result
                        .instructions(&function.instructions, &function.parameters)
                        .is_some_and(|types| scalar(types[function.result.0]))
                {
                    result.scalar_borrowing[i] = false;
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }
    pub fn function(&self, index: usize) -> bool {
        self.scalar_borrowing.get(index).copied().unwrap_or(false)
    }
    pub fn instruction(&self, instruction: &Instruction, inputs: &[Type]) -> bool {
        scalar(instruction.ty)
            && match &instruction.operation {
                Operation::Constant(_)
                | Operation::Boolean(_)
                | Operation::Length(_)
                | Operation::Index { .. }
                | Operation::Binary { .. }
                | Operation::Compare { .. } => true,
                Operation::Intrinsic {
                    op: Intrinsic::Parse,
                    ..
                } => true,
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
                        && self.identity_body(body, &state)
                        && self.region(finish, &state)
                }
                _ => false,
            }
    }
    pub fn region(&self, region: &Region, inputs: &[Type]) -> bool {
        let Some(types) = self.instructions(&region.instructions, inputs) else {
            return false;
        };
        region.results.iter().all(|id| scalar(types[id.0]))
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
    fn identity_body(&self, body: &Region, state: &[Type]) -> bool {
        self.instructions(&body.instructions, state).is_some()
            && state
                .iter()
                .enumerate()
                .all(|(i, ty)| scalar(*ty) || body.results[i] == crate::ValueId(i))
    }
}
fn scalar(ty: Type) -> bool {
    matches!(ty, Type::I64 | Type::Bool)
}
