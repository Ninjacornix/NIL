//! Syntax-independent typed semantics. Only validated programs can be executed.
pub mod borrowing;
pub mod diagnostic;
mod intrinsic;
pub mod liveness;
pub mod parameters;
pub mod plugin;
pub mod records;
pub mod sums;
mod validate;
pub use intrinsic::Intrinsic;
/// Legacy small allocation guard, retained for existing test/tool callers.
/// This is not a language maximum or a deterministic quota.
pub const MAX_DYNAMIC_BYTES: usize = 64 * 1024 * 1024;
/// Default guard for execution-managed dynamic storage, not process RSS.
pub const DEFAULT_MEMORY_BYTES: usize = 256 * 1024 * 1024;
pub use diagnostic::{Diagnostic, Phase, Span};
pub use records::{RecordDefinition, RecordField};
pub use sums::{SumDefinition, SumVariant};
pub use validate::{operation_type, operation_type_with_records, validate, value_type};
pub const MAX_REGION_DEPTH: usize = 32;
pub const MAX_ARRAY_LEN: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    I64,
    U64,
    U128,
    F64,
    Bool,
    Array(usize),
    Buffer,
    Bytes,
    MapI64,
    MapBytes,
    /// Insertion-ordered set of i64 (ADR 051). Keys are integers, so no key is
    /// formatted to bytes on the membership path.
    SetI64,
    /// Integer-keyed, integer-valued map (ADR 051 part 2, `!imap`).
    IntMap,
    Record(usize, usize),
    MapRecord(usize, usize),
    RecordBuffer(usize, usize),
    /// Nominal sum (ADR 050). One opaque pointer-sized slot: a nullary variant
    /// is unboxed, a payload variant is a heap node on its own list.
    ///
    /// Deliberately **not** `is_dynamic`: dynamic means root-managed arena
    /// storage, and rooting a list head would retain every child transitively,
    /// which is O(n) per root store and recursive. A sum is an immutable opaque
    /// word to every analysis that asks.
    Sum(usize),
}

impl Type {
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::I64 | Self::U64 | Self::U128 | Self::F64)
    }
    pub fn is_dynamic(self) -> bool {
        matches!(
            self,
            Self::Buffer
                | Self::Bytes
                | Self::MapI64
                | Self::MapBytes
                | Self::SetI64
                | Self::IntMap
                | Self::MapRecord(..)
                | Self::RecordBuffer(..)
        )
    }
    /// Key type of a keyed collection: bytes for the byte-keyed maps, i64 for
    /// the integer set (ADR 051).
    pub fn key_type(self) -> Option<Self> {
        match self {
            Self::MapI64 | Self::MapBytes | Self::MapRecord(..) => Some(Self::Bytes),
            Self::SetI64 | Self::IntMap => Some(Self::I64),
            _ => None,
        }
    }
    pub fn map_value(self) -> Option<Self> {
        match self {
            Self::MapI64 | Self::IntMap => Some(Self::I64),
            Self::MapBytes => Some(Self::Bytes),
            Self::MapRecord(id, slots) => Some(Self::Record(id, slots)),
            _ => None,
        }
    }

    /// Private tooling slots: dynamic entries use one opaque slot. Not a public ABI.
    pub fn slots(self) -> usize {
        match self {
            Self::I64
            | Self::Bool
            | Self::Buffer
            | Self::Bytes
            | Self::MapI64
            | Self::MapBytes
            | Self::SetI64
            | Self::IntMap => 1,
            Self::Array(len) => len,
            Self::U64 | Self::F64 => 1,
            Self::U128 => 2,
            Self::Record(_, slots) => slots,
            Self::MapRecord(..) | Self::RecordBuffer(..) | Self::Sum(_) => 1,
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
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    BitNot,
    Abs,
    Sqrt,
    Floor,
    Ceil,
    Trunc,
    Round,
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
    PluginCall {
        provider: Box<plugin::Provider>,
        arguments: Vec<ValueId>,
    },
    Record {
        ty: Type,
        fields: Vec<ValueId>,
    },
    Field {
        record: ValueId,
        field: usize,
    },
    UpdateField {
        record: ValueId,
        field: usize,
        value: ValueId,
    },
    /// Build a sum value. A nullary variant takes no fields and never allocates.
    Construct {
        ty: Type,
        variant: usize,
        fields: Vec<ValueId>,
    },
    /// Variant index of a sum value, as i64. Total on every sum value.
    SumTag {
        value: ValueId,
        /// Expected sum type. Variant names resolve globally, so without this
        /// a match whose arms belong to another sum could typecheck silently.
        ty: Type,
    },
    /// Read one payload slot after checking that the value has `variant`.
    SumField {
        value: ValueId,
        ty: Type,
        variant: usize,
        slot: usize,
    },
    RecordMap(Type),
    RecordBuffer {
        ty: Type,
        length: ValueId,
        fill: ValueId,
    },
    Bytes(Vec<u8>),
    Intrinsic {
        op: Intrinsic,
        arguments: Vec<ValueId>,
    },
    Constant(i64),
    Unsigned {
        value: u128,
        ty: Type,
    },
    Float(u64),
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
    Unary {
        op: UnaryOp,
        value: ValueId,
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
    pub records: Vec<RecordDefinition>,
    pub sums: Vec<SumDefinition>,
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
