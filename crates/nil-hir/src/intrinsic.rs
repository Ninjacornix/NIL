//! Provisional typed application instruction set; names are independent of syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    Sort,
    ToI64,
    ToU64,
    ToU128,
    ToF64,
    TruncI64,
    TruncU64,
    Bits,
    FloatBits,
    ParseU64,
    ParseU128,
    ParseF64,

    Map,
    ByteMap,
    Insert,
    Put,
    Get,
    Has,
    Size,
    Key,
    Buffer,
    Bytes,
    Concat,
    Slice,
    Format,
    Parse,
    ParseBuffer,
    Equal,
    Find,
    Read,
    Write,
    Out,
}
impl Intrinsic {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "sort" => Self::Sort,
            "i64" => Self::ToI64,
            "u64" => Self::ToU64,
            "u128" => Self::ToU128,
            "f64" => Self::ToF64,
            "trunci64" => Self::TruncI64,
            "truncu64" => Self::TruncU64,
            "bits" => Self::Bits,
            "floatbits" => Self::FloatBits,
            "parseu64" => Self::ParseU64,
            "parseu128" => Self::ParseU128,
            "parsef64" => Self::ParseF64,

            "map" => Self::Map,
            "bytemap" => Self::ByteMap,
            "insert" => Self::Insert,
            "put" => Self::Put,
            "get" => Self::Get,
            "has" => Self::Has,
            "size" => Self::Size,
            "key" => Self::Key,
            "buffer" => Self::Buffer,
            "bytes" => Self::Bytes,
            "concat" => Self::Concat,
            "slice" => Self::Slice,
            "format" => Self::Format,
            "parse" => Self::Parse,
            "parsebuf" => Self::ParseBuffer,
            "equal" => Self::Equal,
            "find" => Self::Find,
            "read" => Self::Read,
            "write" => Self::Write,
            "out" => Self::Out,
            _ => return None,
        })
    }
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::ToI64
                | Self::ToU64
                | Self::ToU128
                | Self::ToF64
                | Self::TruncI64
                | Self::TruncU64
                | Self::Bits
                | Self::FloatBits
                | Self::ParseU64
                | Self::ParseU128
                | Self::ParseF64
        )
    }
    pub fn has_host_effect(self) -> bool {
        matches!(self, Self::Read | Self::Write | Self::Out)
    }
}
