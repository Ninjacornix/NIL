//! Version 1 compiler-visible semantic boundary. No foreign/native loading.
use crate::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: u32,
    pub operation: u32,
    entry: FunctionId,
    program: ValidatedProgram,
}
impl Provider {
    pub fn new(
        id: u32,
        operation: u32,
        program: ValidatedProgram,
        entry: FunctionId,
    ) -> Result<Self, Diagnostic> {
        if program.program().functions.len() > 128
            || program.program().functions.get(entry.0).is_none()
        {
            return Err(error("invalid plugin entry or function count"));
        }
        let proof = borrowing::Summaries::analyze(&program);
        if (0..program.program().functions.len()).any(|i| !proof.function(i)) {
            return Err(error(
                "borrow declaration is not proved: allocation or host effect",
            ));
        }
        let mut visiting = vec![false; program.program().functions.len()];
        let mut done = visiting.clone();
        for index in 0..visiting.len() {
            acyclic(index, &program, &mut visiting, &mut done)?;
        }
        Ok(Self {
            id,
            operation,
            entry,
            program,
        })
    }
    pub fn program(&self) -> &ValidatedProgram {
        &self.program
    }
    pub fn entry(&self) -> FunctionId {
        self.entry
    }
    pub fn signature(&self) -> &Function {
        &self.program.program().functions[self.entry.0]
    }
}
pub fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E024", Phase::Check, None, message)
}
fn acyclic(
    index: usize,
    program: &ValidatedProgram,
    visiting: &mut [bool],
    done: &mut [bool],
) -> Result<(), Diagnostic> {
    if done[index] {
        return Ok(());
    }
    if visiting[index] {
        return Err(error("recursive plugin providers are not supported"));
    }
    visiting[index] = true;
    fn instructions(
        list: &[Instruction],
        program: &ValidatedProgram,
        visiting: &mut [bool],
        done: &mut [bool],
    ) -> Result<(), Diagnostic> {
        for instruction in list {
            match &instruction.operation {
                Operation::PluginCall { .. } => {
                    return Err(error("nested plugin providers are not supported"));
                }
                Operation::Call { function, .. } => acyclic(function.0, program, visiting, done)?,
                Operation::If {
                    then_region,
                    else_region,
                    ..
                } => {
                    instructions(&then_region.instructions, program, visiting, done)?;
                    instructions(&else_region.instructions, program, visiting, done)?;
                }
                Operation::Loop {
                    condition,
                    body,
                    finish,
                    ..
                } => {
                    for region in [condition, body, finish] {
                        instructions(&region.instructions, program, visiting, done)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    instructions(
        &program.program().functions[index].instructions,
        program,
        visiting,
        done,
    )?;
    visiting[index] = false;
    done[index] = true;
    Ok(())
}
#[path = "../../../plugins/sequence/provider.rs"]
mod sequence;
/// Compatibility input is normalized; equality has no core execution path.
pub(crate) fn normalize(program: &mut Program) {
    fn list(instructions: &mut [Instruction], inputs: &[Type], records: &[RecordDefinition]) {
        let mut types = inputs.to_vec();
        for instruction in instructions {
            match &mut instruction.operation {
                Operation::Intrinsic {
                    op: Intrinsic::Equal,
                    arguments,
                } => {
                    let provider = sequence::equality(types[arguments[0].0], records);
                    instruction.operation = Operation::PluginCall {
                        provider: Box::new(provider),
                        arguments: arguments.clone(),
                    };
                }
                Operation::If {
                    then_region,
                    else_region,
                    ..
                } => {
                    list(&mut then_region.instructions, &types, records);
                    list(&mut else_region.instructions, &types, records);
                }
                Operation::Loop {
                    initial,
                    condition,
                    body,
                    finish,
                } => {
                    let state = initial.iter().map(|id| types[id.0]).collect::<Vec<_>>();
                    for region in [condition, body, finish] {
                        list(&mut region.instructions, &state, records);
                    }
                }
                _ => {}
            }
            types.push(instruction.ty);
        }
    }
    for function in &mut program.functions {
        list(
            &mut function.instructions,
            &function.parameters,
            &program.records,
        );
    }
}

pub fn providers(program: &ValidatedProgram) -> Vec<&Provider> {
    fn walk<'a>(list: &'a [Instruction], result: &mut Vec<&'a Provider>) {
        for i in list {
            match &i.operation {
                Operation::PluginCall { provider, .. } => {
                    if !result.contains(&provider.as_ref()) {
                        result.push(provider);
                    }
                }
                Operation::If {
                    then_region,
                    else_region,
                    ..
                } => {
                    walk(&then_region.instructions, result);
                    walk(&else_region.instructions, result);
                }
                Operation::Loop {
                    condition,
                    body,
                    finish,
                    ..
                } => {
                    for r in [condition, body, finish] {
                        walk(&r.instructions, result);
                    }
                }
                _ => {}
            }
        }
    }
    let mut result = vec![];
    for f in &program.program().functions {
        walk(&f.instructions, &mut result);
    }
    result
}
