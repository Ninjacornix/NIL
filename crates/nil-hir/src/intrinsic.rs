//! Provisional typed application instruction set; names are independent of syntax.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    Sort,
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
    pub fn has_host_effect(self) -> bool {
        matches!(self, Self::Read | Self::Write | Self::Out)
    }
}
