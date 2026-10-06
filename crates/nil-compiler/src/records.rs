//! Private scalar-record map encoding. No dynamic leaf can enter this format.
use crate::evaluator::Value;
use nil_hir::{RecordDefinition, Type};
use std::sync::Arc;
pub fn pack(value: &Value, out: &mut Vec<u8>) {
    let mut word = |v: u64| out.extend_from_slice(&v.to_le_bytes());
    match value {
        Value::I64(v) => word(*v as u64),
        Value::U64(v) | Value::F64(v) => word(*v),
        Value::U128(v) => {
            word(*v as u64);
            word((*v >> 64) as u64);
        }
        Value::Bool(v) => word(u64::from(*v)),
        Value::Array(v) => {
            for v in v.iter() {
                word(*v as u64);
            }
        }
        Value::Record(_, fields) => {
            for f in fields.iter() {
                pack(f, out);
            }
        }
        _ => unreachable!("validated scalar-record map"),
    }
}
pub fn unpack(ty: Type, records: &[RecordDefinition], bytes: &mut &[u8]) -> Value {
    fn word(bytes: &mut &[u8]) -> u64 {
        let (word, rest) = bytes.split_at(8);
        *bytes = rest;
        u64::from_le_bytes(word.try_into().expect("validated packed record"))
    }
    match ty {
        Type::I64 => Value::I64(word(bytes) as i64),
        Type::U64 => Value::U64(word(bytes)),
        Type::F64 => Value::F64(word(bytes)),
        Type::U128 => {
            let low = word(bytes) as u128;
            Value::U128(low | ((word(bytes) as u128) << 64))
        }
        Type::Bool => Value::Bool(word(bytes) != 0),
        Type::Array(n) => Value::Array(
            (0..n)
                .map(|_| word(bytes) as i64)
                .collect::<Vec<_>>()
                .into(),
        ),
        Type::Record(id, _) => Value::Record(
            ty,
            Arc::from(
                records[id]
                    .fields
                    .iter()
                    .map(|f| unpack(f.ty, records, bytes))
                    .collect::<Vec<_>>(),
            ),
        ),
        _ => unreachable!("validated scalar-record map"),
    }
}
