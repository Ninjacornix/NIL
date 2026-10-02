//! Syntax-independent typed semantics. Only validated programs can be executed.
pub mod diagnostic;
mod intrinsic;
mod validate;
pub use intrinsic::Intrinsic;
pub const MAX_DYNAMIC_BYTES: usize = 64 * 1024 * 1024;
pub use diagnostic::{Diagnostic, Phase, Span};
pub use validate::{operation_type, validate, value_type};
pub const MAX_REGION_DEPTH: usize = 32;
pub const MAX_ARRAY_LEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I64,
    Bool,
    Array(usize),
    Buffer,
    Bytes,
}

impl Type {
    /// Private tooling slots: dynamic entries use one opaque slot. Not a public ABI.
    pub fn slots(self) -> usize {
        match self {
            Self::I64 | Self::Bool | Self::Buffer | Self::Bytes => 1,
            Self::Array(len) => len,
        }
    }
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
    Bytes(Vec<u8>),
    Intrinsic {
        op: Intrinsic,
        arguments: Vec<ValueId>,
    },
    Constant(i64),
    Boolean(bool),
    Array(Vec<ValueId>),
    Repeat {
        value: ValueId,
        len: usize,
    },
    Length(ValueId),
    Index {
        array: ValueId,
        index: ValueId,
    },
    Replace {
        array: ValueId,
        index: ValueId,
        value: ValueId,
    },
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
