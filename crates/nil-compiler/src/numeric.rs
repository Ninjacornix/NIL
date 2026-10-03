//! Deterministic scalar numeric semantics; source syntax has no role here.
use crate::evaluator::Value;
use nil_hir::{BinaryOp, CompareOp, Diagnostic, Intrinsic, Span};

pub const NAN_BITS: u64 = 0x7ff8000000000000;
pub fn canonical_bits(value: f64) -> u64 {
    if value.is_nan() {
        NAN_BITS
    } else {
        value.to_bits()
    }
}
pub fn parse_float(text: &str) -> Option<f64> {
    if matches!(text, "nan" | "inf" | "-inf") {
        return text.parse().ok();
    }
    let b = text.as_bytes();
    let mut i = usize::from(matches!(b.first(), Some(b'+' | b'-')));
    let first = i;
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    if i == first {
        return None;
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let first = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == first {
            return None;
        }
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let first = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == first {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    text.parse().ok()
}
pub fn format_float(bits: u64) -> String {
    let f = f64::from_bits(bits);
    if f.is_nan() {
        "nan".into()
    } else if f == f64::INFINITY {
        "inf".into()
    } else if f == f64::NEG_INFINITY {
        "-inf".into()
    } else {
        format!("{f:.16e}")
    }
}
pub fn format(value: &Value) -> String {
    match value {
        Value::I64(v) => v.to_string(),
        Value::U64(v) => v.to_string(),
        Value::U128(v) => v.to_string(),
        Value::F64(v) => format_float(*v),
        _ => unreachable!("numeric format"),
    }
}
fn fault(code: &'static str, span: Option<Span>) -> Diagnostic {
    crate::application::fault(
        code,
        span,
        if code == "E021" {
            "numeric conversion out of range"
        } else {
            "invalid numeric text"
        },
    )
}
fn unsigned(v: &Value, span: Option<Span>) -> Result<u128, Diagnostic> {
    match v {
        Value::I64(v) => u128::try_from(*v).map_err(|_| fault("E021", span)),
        Value::U64(v) => Ok(*v as u128),
        Value::U128(v) => Ok(*v),
        _ => unreachable!(),
    }
}
pub fn intrinsic(op: Intrinsic, a: &Value, span: Option<Span>) -> Result<Value, Diagnostic> {
    use Intrinsic::*;
    Ok(match op {
        Bits => Value::U64(match a {
            Value::F64(v) => *v,
            _ => unreachable!(),
        }),
        FloatBits => Value::F64(canonical_bits(f64::from_bits(match a {
            Value::U64(v) => *v,
            _ => unreachable!(),
        }))),
        ParseF64 => Value::F64(canonical_bits(
            parse_float(std::str::from_utf8(a.bytes()).map_err(|_| fault("E022", span))?)
                .ok_or_else(|| fault("E022", span))?,
        )),
        ParseU64 | ParseU128 => {
            let text = std::str::from_utf8(a.bytes()).map_err(|_| fault("E022", span))?;
            if text.is_empty()
                || !text.bytes().all(|v| v.is_ascii_digit())
                || (text.len() > 1 && text.starts_with('0'))
            {
                return Err(fault("E022", span));
            }
            let n = text.parse::<u128>().map_err(|_| fault("E022", span))?;
            if op == ParseU64 {
                Value::U64(u64::try_from(n).map_err(|_| fault("E022", span))?)
            } else {
                Value::U128(n)
            }
        }
        ToF64 => Value::F64(canonical_bits(match a {
            Value::I64(v) => *v as f64,
            Value::U64(v) => *v as f64,
            Value::U128(v) => *v as f64,
            Value::F64(v) => f64::from_bits(*v),
            _ => unreachable!(),
        })),
        TruncI64 | TruncU64 => {
            let n = match a {
                Value::I64(v) => *v as u64,
                Value::U64(v) => *v,
                Value::U128(v) => *v as u64,
                _ => unreachable!(),
            };
            if op == TruncI64 {
                Value::I64(n as i64)
            } else {
                Value::U64(n)
            }
        }
        ToI64 => {
            let n = match a {
                Value::I64(v) => *v,
                Value::F64(v) => {
                    let f = f64::from_bits(*v);
                    if !f.is_finite()
                        || !(-9223372036854775808.0..9223372036854775808.0).contains(&f)
                    {
                        return Err(fault("E021", span));
                    }
                    f as i64
                }
                _ => i64::try_from(unsigned(a, span)?).map_err(|_| fault("E021", span))?,
            };
            Value::I64(n)
        }
        ToU64 | ToU128 => {
            let n = if let Value::F64(v) = a {
                let f = f64::from_bits(*v);
                let limit = if op == ToU64 {
                    18446744073709551616.0
                } else {
                    f64::from_bits(0x47f0000000000000)
                };
                if !f.is_finite() || !(0.0..limit).contains(&f) {
                    return Err(fault("E021", span));
                }
                f as u128
            } else {
                unsigned(a, span)?
            };
            if op == ToU64 {
                Value::U64(u64::try_from(n).map_err(|_| fault("E021", span))?)
            } else {
                Value::U128(n)
            }
        }
        _ => unreachable!("numeric intrinsic"),
    })
}
pub fn binary(op: BinaryOp, a: &Value, b: &Value, span: Option<Span>) -> Result<Value, Diagnostic> {
    if let (Value::F64(a), Value::F64(b)) = (a, b) {
        let (a, b) = (f64::from_bits(*a), f64::from_bits(*b));
        return Ok(Value::F64(canonical_bits(match op {
            BinaryOp::Add => a + b,
            BinaryOp::Sub => a - b,
            BinaryOp::Mul => a * b,
            BinaryOp::Div => a / b,
        })));
    }
    let aa = unsigned(a, span)?;
    let bb = unsigned(b, span)?;
    let n = match op {
        BinaryOp::Add => aa.wrapping_add(bb),
        BinaryOp::Sub => aa.wrapping_sub(bb),
        BinaryOp::Mul => aa.wrapping_mul(bb),
        BinaryOp::Div => {
            if bb == 0 {
                return Err(crate::application::fault("E009", span, "division by zero"));
            }
            aa / bb
        }
    };
    Ok(if matches!(a, Value::U64(_)) {
        Value::U64(n as u64)
    } else {
        Value::U128(n)
    })
}
pub fn compare(op: CompareOp, a: &Value, b: &Value) -> bool {
    fn cmp<T: PartialOrd>(op: CompareOp, a: T, b: T) -> bool {
        match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            CompareOp::Lt => a < b,
            CompareOp::Le => a <= b,
            CompareOp::Gt => a > b,
            CompareOp::Ge => a >= b,
        }
    }
    match (a, b) {
        (Value::F64(a), Value::F64(b)) => cmp(op, f64::from_bits(*a), f64::from_bits(*b)),
        (Value::U64(a), Value::U64(b)) => cmp(op, *a, *b),
        (Value::U128(a), Value::U128(b)) => cmp(op, *a, *b),
        _ => unreachable!(),
    }
}
