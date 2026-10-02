//! Provisional typed application instruction set; names are independent of syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    Buffer,
    Bytes,
    Concat,
    Slice,
    Format,
    Parse,
    Read,
    Write,
    Out,
}
impl Intrinsic {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "buffer" => Self::Buffer,
            "bytes" => Self::Bytes,
            "concat" => Self::Concat,
            "slice" => Self::Slice,
            "format" => Self::Format,
            "parse" => Self::Parse,
            "read" => Self::Read,
            "write" => Self::Write,
            "out" => Self::Out,
            _ => return None,
        })
    }
    pub fn has_host_effect(self) -> bool {
        matches!(self, Self::Read | Self::Write | Self::Out)
    }
}
