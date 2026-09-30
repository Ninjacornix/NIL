//! Syntax-independent M1/M2 semantics. Only validated programs can be executed.
pub mod diagnostic;
mod validate;
pub use diagnostic::{Diagnostic, Phase, Span};
pub use validate::{operation_type, validate, value_type};
pub const MAX_REGION_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I64,
    Bool,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub instructions: Vec<Instruction>,
    pub results: Vec<ValueId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Constant(i64),
    Boolean(bool),
    Compare {
        op: CompareOp,
        lhs: ValueId,
        rhs: ValueId,
    },
    If {
        condition: ValueId,
        then_region: Region,
        else_region: Region,
    },
    Loop {
        initial: Vec<ValueId>,
        condition: Region,
        body: Region,
        finish: Region,
    },
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

/// Arithmetic is explicit semantic metadata, independent of surface syntax.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arithmetic {
    #[default]
    Checked,
    Wrapping,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    pub arithmetic: Arithmetic,
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
