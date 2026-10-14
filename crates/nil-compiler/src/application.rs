//! Explicit host boundary and allocation accounting for the application profile.
use crate::evaluator::Value;
use nil_hir::{DEFAULT_MEMORY_BYTES, Diagnostic, Intrinsic, Phase, Span};
use std::io::{Read, Write};

pub trait Host {
    fn env(&mut self, _name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        Err(denied())
    }
    fn random(&mut self) -> Result<u64, Diagnostic> {
        Err(denied())
    }
    fn directory(&mut self, _path: &[u8]) -> Result<Vec<Vec<u8>>, Diagnostic> {
        Err(denied())
    }
    fn read(&mut self, _path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        Err(denied())
    }
    /// Read at most the admitted payload plus one excess witness byte.
    /// Streaming Hosts must override this method to stop before later I/O errors.
    /// This default preserves compatibility for already-materialized fixtures.
    fn read_with_limit(&mut self, path: &[u8], limit: usize) -> Result<Vec<u8>, Diagnostic> {
        let data = self.read(path)?;
        if data.len() > limit {
            Err(fault("E013", None, "dynamic allocation limit exceeded"))
        } else {
            Ok(data)
        }
    }
    fn write(&mut self, _path: &[u8], _data: &[u8]) -> Result<(), Diagnostic> {
        Err(denied())
    }
    fn out(&mut self, _data: &[u8]) -> Result<(), Diagnostic> {
        Err(denied())
    }
}
fn denied() -> Diagnostic {
    Diagnostic::new("E018", Phase::Execute, None, "host I/O is not enabled")
}
pub struct DeniedHost;
impl Host for DeniedHost {}
/// Explicit opt-in to the caller's filesystem/stdout permissions; not a sandbox.
#[derive(Default)]
pub struct FileHost {
    random_state: Option<u64>,
}

/// Injectable entropy state; forwarding does not grant other capabilities.
pub struct SeededHost<H> {
    pub inner: H,
    state: u64,
}
impl<H> SeededHost<H> {
    pub fn new(inner: H, seed: u64) -> Self {
        Self { inner, state: seed }
    }
}
pub fn next_random(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}
impl<H: Host> Host for SeededHost<H> {
    fn random(&mut self) -> Result<u64, Diagnostic> {
        Ok(next_random(&mut self.state))
    }
    fn env(&mut self, name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        self.inner.env(name)
    }
    fn directory(&mut self, path: &[u8]) -> Result<Vec<Vec<u8>>, Diagnostic> {
        self.inner.directory(path)
    }
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        self.inner.read(path)
    }
    fn read_with_limit(&mut self, path: &[u8], limit: usize) -> Result<Vec<u8>, Diagnostic> {
        self.inner.read_with_limit(path, limit)
    }
    fn write(&mut self, path: &[u8], data: &[u8]) -> Result<(), Diagnostic> {
        self.inner.write(path, data)
    }
    fn out(&mut self, data: &[u8]) -> Result<(), Diagnostic> {
        self.inner.out(data)
    }
}
fn os_bytes(value: std::ffi::OsString) -> Result<Vec<u8>, Diagnostic> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(value.into_vec())
    }
    #[cfg(not(unix))]
    {
        value
            .into_string()
            .map(String::into_bytes)
            .map_err(|_| fault("E017", None, "invalid OS encoding"))
    }
}
fn environment_name(name: &[u8]) -> Result<(), Diagnostic> {
    if name.is_empty() || name.contains(&0) || name.contains(&b'=') {
        Err(fault("E017", None, "invalid environment name"))
    } else {
        Ok(())
    }
}
impl Host for FileHost {
    fn env(&mut self, name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        environment_name(name)?;
        let name = path_name(name)?.into_os_string();
        std::env::var_os(name)
            .map(os_bytes)
            .transpose()
            .map(|v| v.unwrap_or_default())
    }
    fn random(&mut self) -> Result<u64, Diagnostic> {
        if self.random_state.is_none() {
            let mut bytes = [0; 8];
            std::fs::File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut bytes))
                .map_err(|_| fault("E015", None, "entropy acquisition failed"))?;
            self.random_state = Some(u64::from_le_bytes(bytes));
        }
        Ok(next_random(self.random_state.as_mut().unwrap()))
    }
    fn directory(&mut self, path: &[u8]) -> Result<Vec<Vec<u8>>, Diagnostic> {
        std::fs::read_dir(path_name(path)?)
            .map_err(|_| fault("E015", None, "directory read failed"))?
            .map(|entry| {
                entry
                    .map_err(|_| fault("E015", None, "directory read failed"))
                    .and_then(|entry| os_bytes(entry.file_name()))
            })
            .collect()
    }
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        self.read_with_limit(path, DEFAULT_MEMORY_BYTES)
    }
    fn read_with_limit(&mut self, path: &[u8], limit: usize) -> Result<Vec<u8>, Diagnostic> {
        let file = std::fs::File::open(path_name(path)?)
            .map_err(|_| fault("E015", None, "file read failed"))?;
        read_prefix(file, limit)
    }
    fn write(&mut self, path: &[u8], data: &[u8]) -> Result<(), Diagnostic> {
        std::fs::write(path_name(path)?, data).map_err(|_| fault("E015", None, "file write failed"))
    }
    fn out(&mut self, data: &[u8]) -> Result<(), Diagnostic> {
        std::io::stdout()
            .write_all(data)
            .map_err(|_| fault("E015", None, "stdout write failed"))
    }
}
/// Bounded streaming acquisition; E013 precedes an error beyond the first excess
/// byte (ADR 033 amendment). No further read is issued after that witness.
pub fn read_prefix(reader: impl Read, limit: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut data = Vec::new();
    let result = reader
        .take((limit as u64).saturating_add(1))
        .read_to_end(&mut data);
    if data.len() > limit {
        return Err(fault("E013", None, "dynamic allocation limit exceeded"));
    }
    result.map_err(|_| fault("E015", None, "file read failed"))?;
    Ok(data)
}
fn path_name(path: &[u8]) -> Result<std::path::PathBuf, Diagnostic> {
    if path.contains(&0) {
        return Err(fault("E017", None, "path contains NUL"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(std::path::PathBuf::from(std::ffi::OsStr::from_bytes(path)))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(path)
            .map(std::path::PathBuf::from)
            .map_err(|_| fault("E017", None, "invalid path encoding"))
    }
}
pub(crate) fn fault(code: &'static str, span: Option<Span>, message: &str) -> Diagnostic {
    Diagnostic::new(code, Phase::Execute, span, message)
}
pub(crate) const SEQUENCE_OVERHEAD: usize = 40;
/// Backend-specific storage estimate. Resource exhaustion is inconclusive in
/// differential testing; this is not an abstract-machine charge ledger.
pub(crate) struct Memory {
    pub bytes: usize,
    pub limit: usize,
}
impl Memory {
    pub fn new(limit: usize) -> Self {
        Self {
            bytes: 0,
            limit: if limit == 0 { usize::MAX } else { limit },
        }
    }
    pub fn grow(&mut self, bytes: usize, span: Option<Span>) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| fault("E013", span, "dynamic allocation guard exceeded"))?;
        Ok(())
    }
}
pub(crate) fn charge(
    used: &mut Memory,
    bytes: usize,
    span: Option<Span>,
) -> Result<(), Diagnostic> {
    used.bytes = used
        .bytes
        .checked_add(bytes)
        .and_then(|n| n.checked_add(SEQUENCE_OVERHEAD))
        .filter(|n| *n <= used.limit)
        .ok_or_else(|| fault("E013", span, "dynamic allocation guard exceeded"))?;
    Ok(())
}
pub(crate) fn intrinsic(
    op: Intrinsic,
    args: &[&Value],
    used: &mut Memory,
    host: &mut dyn Host,
    span: Option<Span>,
) -> Result<Value, Diagnostic> {
    let integer = |i: usize| args[i].integer();
    let bytes = |i: usize| args[i].bytes();
    let result = match op {
        Intrinsic::ToI64
        | Intrinsic::ToU64
        | Intrinsic::ToU128
        | Intrinsic::ToF64
        | Intrinsic::TruncI64
        | Intrinsic::TruncU64
        | Intrinsic::Bits
        | Intrinsic::FloatBits
        | Intrinsic::ParseU64
        | Intrinsic::ParseU128
        | Intrinsic::ParseF64 => unreachable!("evaluator handles numeric operations"),
        Intrinsic::IntMap => {
            let map = crate::keyed::Map::empty_int_map();
            charge(used, map.capacity(), span)?;
            Value::Map(map)
        }
        Intrinsic::Set => {
            let mut set = crate::keyed::Map::empty_set();
            charge(used, set.capacity(), span)?;
            if let Some(Value::Buffer(source)) = args.first() {
                // Insertion order is first occurrence, so a repeat is a no-op.
                for element in source.iter() {
                    set.update(
                        &crate::keyed::Map::integer_key(*element),
                        crate::keyed::EntryValue::Integer(0),
                        false,
                        used,
                        span,
                    )?;
                }
            }
            Value::Map(set)
        }
        Intrinsic::Add | Intrinsic::Remove => {
            let Value::Map(set) = args[0] else {
                unreachable!("validated set")
            };
            let mut set = set.clone();
            let key = crate::keyed::Map::integer_key(integer(1));
            if op == Intrinsic::Remove {
                set.remove(&key);
            } else {
                set.update(
                    &key,
                    crate::keyed::EntryValue::Integer(0),
                    false,
                    used,
                    span,
                )?;
            }
            Value::Map(set)
        }
        Intrinsic::Map | Intrinsic::ByteMap => {
            let map = crate::keyed::Map::empty(op == Intrinsic::ByteMap);
            charge(used, map.capacity(), span)?;
            Value::Map(map)
        }
        Intrinsic::Has | Intrinsic::Size | Intrinsic::Key | Intrinsic::Get => {
            let Value::Map(map) = args[0] else {
                unreachable!("validated map")
            };
            match op {
                Intrinsic::Size => Value::I64(map.entries().len() as i64),
                // Sets and the integer map key on the integer directly; the
                // byte maps key on bytes.
                Intrinsic::Has if map.ty().key_type() == Some(nil_hir::Type::I64) => Value::Bool(
                    map.get(&crate::keyed::Map::integer_key(integer(1)))
                        .is_some(),
                ),
                Intrinsic::Has => Value::Bool(map.get(bytes(1)).is_some()),
                Intrinsic::Key => {
                    let i = integer(1);
                    let entry = usize::try_from(i)
                        .ok()
                        .and_then(|i| map.entries().get(i))
                        .ok_or_else(|| fault("E012", span, "map iteration index out of bounds"))?;
                    // Iteration yields the key in its declared type, so the
                    // integer map hands back an i64 rather than eight bytes.
                    if map.ty() == nil_hir::Type::IntMap {
                        let mut be = [0u8; 8];
                        be.copy_from_slice(&entry.0);
                        Value::I64(i64::from_be_bytes(be))
                    } else {
                        charge(used, entry.0.len(), span)?;
                        Value::Bytes(entry.0.clone().into())
                    }
                }
                Intrinsic::Get => {
                    let key: Vec<u8> = if map.ty() == nil_hir::Type::IntMap {
                        crate::keyed::Map::integer_key(integer(1)).to_vec()
                    } else {
                        bytes(1).to_vec()
                    };
                    let entry = map
                        .get(&key)
                        .ok_or_else(|| fault("E019", span, "missing map key"))?;
                    match entry {
                        crate::keyed::EntryValue::Integer(v) => Value::I64(*v),
                        crate::keyed::EntryValue::Bytes(v) => {
                            charge(used, v.len(), span)?;
                            Value::Bytes(v.clone().into())
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        Intrinsic::Sort | Intrinsic::Insert | Intrinsic::Put => {
            unreachable!("evaluator handles immutable map updates")
        }
        Intrinsic::Buffer | Intrinsic::Bytes => {
            let n = usize::try_from(integer(0))
                .map_err(|_| fault("E014", span, "invalid sequence length"))?;
            let width = if op == Intrinsic::Buffer { 8 } else { 1 };
            let size = n
                .checked_mul(width)
                .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
            if op == Intrinsic::Bytes && !(0..=255).contains(&integer(1)) {
                return Err(fault("E014", span, "byte value must be 0..255"));
            }
            charge(used, size, span)?;
            if op == Intrinsic::Buffer {
                Value::Buffer(vec![integer(1); n].into())
            } else {
                Value::Bytes(vec![integer(1) as u8; n].into())
            }
        }
        Intrinsic::Concat => {
            let width = if matches!(args[0], Value::Bytes(_)) {
                1
            } else {
                8
            };
            let capacity = concat_capacity(
                args[0].capacity(),
                args[0].len(),
                args[1].len(),
                width,
                used.limit,
                span,
            )?;
            charge(used, capacity * width, span)?;
            match (args[0], args[1]) {
                (Value::Bytes(a), Value::Bytes(b)) => Value::Bytes(a.concat(b, capacity)),
                (Value::Buffer(a), Value::Buffer(b)) => Value::Buffer(a.concat(b, capacity)),
                _ => unreachable!("validated concat"),
            }
        }
        Intrinsic::Slice => {
            let start = usize::try_from(integer(1))
                .map_err(|_| fault("E012", span, "sequence slice out of bounds"))?;
            let len = usize::try_from(integer(2))
                .map_err(|_| fault("E012", span, "sequence slice out of bounds"))?;
            let end = start
                .checked_add(len)
                .filter(|end| *end <= args[0].len())
                .ok_or_else(|| fault("E012", span, "sequence slice out of bounds"))?;
            charge(
                used,
                len * if matches!(args[0], Value::Bytes(_)) {
                    1
                } else {
                    8
                },
                span,
            )?;
            match args[0] {
                Value::Bytes(v) => Value::Bytes(v[start..end].to_vec().into()),
                Value::Buffer(v) => Value::Buffer(v[start..end].to_vec().into()),
                _ => unreachable!(),
            }
        }
        Intrinsic::Format => {
            let text = crate::numeric::format(args[0]).into_bytes();
            charge(used, text.len(), span)?;
            Value::Bytes(text.into())
        }
        Intrinsic::Parse => Value::I64(parse_decimal(bytes(0), span)?),
        Intrinsic::Equal => unreachable!("equality normalized to plugin call"),
        Intrinsic::Find => {
            let start = usize::try_from(integer(2))
                .ok()
                .filter(|start| *start <= args[0].len())
                .ok_or_else(|| fault("E012", span, "sequence search start out of bounds"))?;
            let found = match args[0] {
                Value::Bytes(a) => {
                    let needle = u8::try_from(integer(1))
                        .map_err(|_| fault("E014", span, "byte value must be 0..255"))?;
                    a[start..].iter().position(|v| *v == needle)
                }
                Value::Buffer(a) => a[start..].iter().position(|v| *v == integer(1)),
                _ => unreachable!("validated search"),
            };
            Value::I64(found.map_or(args[0].len(), |i| start + i) as i64)
        }
        Intrinsic::ParseBuffer => {
            let text = bytes(0);
            let mut separators = [false; 256];
            for byte in bytes(1) {
                separators[*byte as usize] = true;
            }
            let count = if text.is_empty() {
                0
            } else {
                text.iter().filter(|b| separators[**b as usize]).count()
                    + usize::from(!separators[text[text.len() - 1] as usize])
            };
            let size = count
                .checked_mul(8)
                .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
            charge(used, size, span)?;
            let mut values = Vec::with_capacity(count);
            let mut start = 0;
            for (end, byte) in text.iter().enumerate() {
                if separators[*byte as usize] {
                    values.push(parse_decimal(&text[start..end], span)?);
                    start = end + 1;
                }
            }
            if start < text.len() {
                values.push(parse_decimal(&text[start..], span)?);
            }
            Value::Buffer(values.into())
        }
        Intrinsic::Random => Value::U64(host.random().map_err(|mut d| {
            d.span = span;
            d
        })?),
        Intrinsic::Env => {
            environment_name(bytes(0)).map_err(|mut d| {
                d.span = span;
                d
            })?;
            let data = host.env(bytes(0)).map_err(|mut d| {
                d.span = span;
                d
            })?;
            charge(used, data.len(), span)?;
            Value::Bytes(data.into())
        }
        Intrinsic::Directory => {
            path_name(bytes(0)).map_err(|mut d| {
                d.span = span;
                d
            })?;
            let names = host.directory(bytes(0)).map_err(|mut d| {
                d.span = span;
                d
            })?;
            let map = crate::keyed::Map::directory(names, used.limit, span)?;
            charge(used, map.capacity(), span)?;
            Value::Map(map)
        }
        Intrinsic::Read => {
            path_name(bytes(0)).map_err(|mut e| {
                e.span = span;
                e
            })?;
            let available = used
                .limit
                .saturating_sub(used.bytes)
                .saturating_sub(SEQUENCE_OVERHEAD);
            let data = host.read_with_limit(bytes(0), available).map_err(|mut e| {
                e.span = span;
                e
            })?;
            charge(used, data.len(), span)?;
            Value::Bytes(data.into())
        }
        Intrinsic::Write => {
            path_name(bytes(0)).map_err(|mut e| {
                e.span = span;
                e
            })?;
            host.write(bytes(0), bytes(1)).map_err(|mut e| {
                e.span = span;
                e
            })?;
            Value::I64(bytes(1).len() as i64)
        }
        Intrinsic::Out => {
            host.out(bytes(0)).map_err(|mut e| {
                e.span = span;
                e
            })?;
            Value::I64(bytes(0).len() as i64)
        }
    };
    Ok(result)
}

// Geometric allocation strategy, not observable language capacity semantics.
pub(crate) fn concat_capacity(
    capacity: usize,
    left: usize,
    right: usize,
    width: usize,
    limit: usize,
    span: Option<Span>,
) -> Result<usize, Diagnostic> {
    let maximum = limit.saturating_sub(SEQUENCE_OVERHEAD) / width;
    let needed = left
        .checked_add(right)
        .filter(|n| *n <= maximum)
        .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
    Ok(if needed <= capacity {
        capacity
    } else {
        needed.max(capacity.saturating_mul(2).min(maximum))
    })
}

fn parse_decimal(bytes: &[u8], span: Option<Span>) -> Result<i64, Diagnostic> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| fault("E016", span, "invalid decimal i64"))?;
    let value = text
        .parse::<i64>()
        .map_err(|_| fault("E016", span, "invalid decimal i64"))?;
    if text != value.to_string() {
        return Err(fault("E016", span, "invalid decimal i64"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geometric_growth_does_not_reserve_a_second_copy() {
        assert_eq!(concat_capacity(1, 1, 1, 1, 1024, None).unwrap(), 2);
        assert_eq!(concat_capacity(4, 3, 1, 1, 1024, None).unwrap(), 4);
        assert_eq!(
            concat_capacity(16777216, 16777216, 1, 1, nil_hir::MAX_DYNAMIC_BYTES, None).unwrap(),
            33554432
        );
        let mut memory = Memory::new(100);
        charge(&mut memory, 50, None).unwrap();
        memory.grow(10, None).unwrap();
        assert_eq!(memory.grow(1, None).unwrap_err().code, "E013");
    }
}
