//! Syntax-independent M1 semantics. Only validated programs can be executed.
pub mod diagnostic;
mod validate;
pub use diagnostic::{Diagnostic, Phase, Span};
pub use validate::validate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Constant(i64),
    Binary {
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    Call {
        function: FunctionId,
        arguments: Vec<ValueId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub operation: Operation,
    pub ty: Type,
    pub span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub parameters: Vec<Type>,
    pub result_type: Type,
    pub instructions: Vec<Instruction>,
    pub result: ValueId,
    pub return_span: Option<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

/// Construct via `validate`. No mutable access to the underlying program is exposed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedProgram(Program);
impl ValidatedProgram {
    pub fn program(&self) -> &Program {
        &self.0
    }
}
