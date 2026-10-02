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
pub(crate) fn charge(used: &mut usize, bytes: usize, span: Option<Span>) -> Result<(), Diagnostic> {
    *used = used
        .checked_add(bytes)
        .and_then(|n| n.checked_add(32))
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
            let n = args[0]
                .len()
                .checked_add(args[1].len())
                .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?;
            let width = if matches!(args[0], Value::Bytes(_)) {
                1
            } else {
                8
            };
            charge(
                used,
                n.checked_mul(width)
                    .ok_or_else(|| fault("E013", span, "dynamic allocation limit exceeded"))?,
                span,
            )?;
            if width == 1 {
                Value::Bytes([bytes(0), bytes(1)].concat().into())
            } else {
                Value::Buffer([args[0].elements(), args[1].elements()].concat().into())
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
        Intrinsic::Parse => {
            let text = std::str::from_utf8(bytes(0))
                .map_err(|_| fault("E016", span, "invalid decimal i64"))?;
            let value = text
                .parse::<i64>()
                .map_err(|_| fault("E016", span, "invalid decimal i64"))?;
            if text != value.to_string() {
                return Err(fault("E016", span, "invalid decimal i64"));
            }
            Value::I64(value)
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
