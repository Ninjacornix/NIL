//! Explicit host boundary and allocation accounting for the application profile.
use crate::evaluator::Value;
use nil_hir::{Diagnostic, Intrinsic, MAX_DYNAMIC_BYTES, Phase, Span};
use std::io::{Read, Write};

pub trait Host {
    fn read(&mut self, _path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        Err(denied())
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
pub struct FileHost;
impl Host for FileHost {
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        let mut bytes = Vec::new();
        std::fs::File::open(path_name(path)?)
            .and_then(|file| {
                file.take(MAX_DYNAMIC_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
            })
            .map_err(|_| fault("E015", None, "file read failed"))?;
        Ok(bytes)
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
pub(crate) fn charge(used: &mut usize, bytes: usize, span: Option<Span>) -> Result<(), Diagnostic> {
    *used = used
        .checked_add(bytes)
        .and_then(|n| n.checked_add(SEQUENCE_OVERHEAD))
        .filter(|n| *n <= MAX_DYNAMIC_BYTES)
        .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
    Ok(())
}
pub(crate) fn intrinsic(
    op: Intrinsic,
    args: &[&Value],
    used: &mut usize,
    host: &mut dyn Host,
    span: Option<Span>,
) -> Result<Value, Diagnostic> {
    let integer = |i: usize| args[i].integer();
    let bytes = |i: usize| args[i].bytes();
    let result = match op {
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
                Intrinsic::Has => Value::Bool(map.get(bytes(1)).is_some()),
                Intrinsic::Key => {
                    let i = integer(1);
                    let entry = usize::try_from(i)
                        .ok()
                        .and_then(|i| map.entries().get(i))
                        .ok_or_else(|| fault("E012", span, "map iteration index out of bounds"))?;
                    charge(used, entry.0.len(), span)?;
                    Value::Bytes(entry.0.clone().into())
                }
                Intrinsic::Get => {
                    let entry = map
                        .get(bytes(1))
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
        Intrinsic::Insert | Intrinsic::Put => {
            unreachable!("evaluator handles immutable map updates")
        }
        Intrinsic::Buffer | Intrinsic::Bytes => {
            let n = usize::try_from(integer(0))
                .map_err(|_| fault("E013", span, "invalid sequence length"))?;
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
                args[1].capacity(),
                width,
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
            let text = integer(0).to_string().into_bytes();
            charge(used, text.len(), span)?;
            Value::Bytes(text.into())
        }
        Intrinsic::Parse => Value::I64(parse_decimal(bytes(0), span)?),
        Intrinsic::Equal => Value::Bool(match (args[0], args[1]) {
            (Value::Bytes(a), Value::Bytes(b)) => a.as_ref() == b.as_ref(),
            (Value::Buffer(a), Value::Buffer(b)) => a.as_ref() == b.as_ref(),
            _ => unreachable!("validated equality"),
        }),
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
        Intrinsic::Read => {
            path_name(bytes(0)).map_err(|mut e| {
                e.span = span;
                e
            })?;
            let data = host.read(bytes(0)).map_err(|mut e| {
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

// Requested capacity is a language execution policy, identical for copying and
// reuse. Grow geometrically only when required, clamping spare capacity to the
// steady old/right/result budget. Required length wins over the spare-capacity
// hint; the full live/transient reservation still decides admission.
pub(crate) fn concat_capacity(
    capacity: usize,
    left: usize,
    right: usize,
    right_capacity: usize,
    width: usize,
    span: Option<Span>,
) -> Result<usize, Diagnostic> {
    let maximum = (MAX_DYNAMIC_BYTES - SEQUENCE_OVERHEAD) / width;
    let needed = left
        .checked_add(right)
        .filter(|n| *n <= maximum)
        .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
    Ok(if needed <= capacity {
        capacity
    } else {
        let steady = MAX_DYNAMIC_BYTES
            .saturating_sub(3 * SEQUENCE_OVERHEAD + right_capacity * width)
            / (2 * width);
        needed.max(capacity.saturating_mul(2).min(steady))
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
    fn geometric_capacity_is_stable_and_clamped_near_the_quota() {
        assert_eq!(concat_capacity(1, 1, 1, 1, 1, None).unwrap(), 2);
        assert_eq!(concat_capacity(4, 3, 1, 1, 1, None).unwrap(), 4);
        let last_geometric = concat_capacity(16777216, 16777216, 1, 1, 1, None).unwrap();
        assert_eq!(last_geometric, 33554371);
        assert_eq!(
            concat_capacity(last_geometric, last_geometric, 1, 1, 1, None).unwrap(),
            33554372
        );
        let mut used = last_geometric + 40 + 1 + 40;
        charge(&mut used, 33554372, None).unwrap();
        let mut used = 33554372 + 40 + 1 + 40;
        assert_eq!(charge(&mut used, 33554373, None).unwrap_err().code, "E013");
    }
}
