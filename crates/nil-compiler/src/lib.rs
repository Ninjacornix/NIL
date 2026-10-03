//! Minimal NIL frontend and reference execution. No native code generation.
pub mod application;
pub mod evaluator;
pub mod expr;
pub mod keyed;
mod lower;
pub mod parser;
pub mod syntax;
pub use lower::dump;
pub use lower::{CompiledProgram, compile, compile_with_profile, lower, lower_with_arithmetic};
pub use nil_hir as hir;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceProfile {
    LinesV0,
    #[default]
    ExprV0,
    ExprV1,
    ExprV2,
    ExprV3,
    ExprV4,
    ExprV5,
}

impl SourceProfile {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "lines-v0" => Some(Self::LinesV0),
            "expr-v0" => Some(Self::ExprV0),
            "expr-v1" => Some(Self::ExprV1),
            "expr-v2" => Some(Self::ExprV2),
            "expr-v3" => Some(Self::ExprV3),
            "expr-v4" => Some(Self::ExprV4),
            "expr-v5" => Some(Self::ExprV5),
            _ => None,
        }
    }
}

pub mod sequence;

pub mod numeric;
