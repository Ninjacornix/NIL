//! Minimal NIL frontend and reference execution. No native code generation.
pub mod evaluator;
pub mod expr;
mod lower;
pub mod parser;
pub mod syntax;
pub use lower::dump;
pub use lower::{CompiledProgram, compile, compile_with_profile, lower};
pub use nil_hir as hir;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceProfile {
    LinesV0,
    #[default]
    ExprV0,
}

impl SourceProfile {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "lines-v0" => Some(Self::LinesV0),
            "expr-v0" => Some(Self::ExprV0),
            _ => None,
        }
    }
}
