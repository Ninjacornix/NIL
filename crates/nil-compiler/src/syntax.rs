//! Provisional common AST: source labels remain unresolved until lowering.
use nil_hir::{BinaryOp, CompareOp, Span, Type};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub functions: Vec<Function>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub label: u32,
    pub span: Span,
    pub parameters: Vec<Type>,
    pub result_type: Type,
    pub instructions: Vec<Instruction>,
    pub result: u32,
    pub return_span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub kind: InstructionKind,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstructionKind {
    Constant(i64),
    Boolean(bool),
    Compare(CompareOp, u32, u32),
    If(u32, Region, Region),
    Loop(Vec<u32>, Region, Region, Region),
    Binary(BinaryOp, u32, u32),
    Call(u32, Vec<u32>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub instructions: Vec<Instruction>,
    pub results: Vec<u32>,
}
