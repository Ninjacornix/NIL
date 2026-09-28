//! Minimal NIL frontend and reference execution. No native code generation.
pub mod evaluator;
mod lower;
pub mod parser;
pub mod syntax;
pub use lower::dump;
pub use lower::{CompiledProgram, compile, lower};
pub use nil_hir as hir;
