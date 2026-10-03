//! Owned homogeneous maps. The reference index is deliberately independent of
//! native hashing; insertion order and semantic capacity are shared contracts.
use crate::application::{charge, fault};
use nil_hir::{Diagnostic, Span, Type};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryValue {
    Integer(i64),
    Bytes(Vec<u8>),
}
impl EntryValue {
    fn bytes_len(&self) -> usize {
        match self {
            Self::Integer(_) => 0,
            Self::Bytes(v) => v.len(),
        }
    }
}
#[derive(Clone, Debug)]
struct Storage {
    entries: Vec<(Vec<u8>, EntryValue)>,
    index: BTreeMap<Vec<u8>, usize>,
    entry_capacity: usize,
    byte_capacity: usize,
    used: usize,
    ty: Type,
}
#[derive(Clone, Debug)]
pub struct Map(Arc<Storage>);
impl PartialEq for Map {
    fn eq(&self, other: &Self) -> bool {
        self.0.ty == other.0.ty && self.0.entries == other.0.entries
    }
}
impl Eq for Map {}
impl Map {
    pub fn empty(byte_values: bool) -> Self {
        Self(Arc::new(Storage {
            entries: vec![],
            index: BTreeMap::new(),
            entry_capacity: 4,
            byte_capacity: 16,
            used: 0,
            ty: if byte_values {
                Type::MapBytes
            } else {
                Type::MapI64
            },
        }))
    }
    /// Deterministic tooling rendering; not a language serialization operation.
    pub fn render(&self) -> String {
        let hex = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
        format!(
            "[{}]",
            self.entries()
                .iter()
                .map(|(k, v)| {
                    let value = match v {
                        EntryValue::Integer(v) => v.to_string(),
                        EntryValue::Bytes(v) => format!("\"{}\"", hex(v)),
                    };
                    format!("[\"{}\",{value}]", hex(k))
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    pub fn ty(&self) -> Type {
        self.0.ty
    }
    pub fn entries(&self) -> &[(Vec<u8>, EntryValue)] {
        &self.0.entries
    }
    pub fn get(&self, key: &[u8]) -> Option<&EntryValue> {
        self.0.index.get(key).map(|i| &self.0.entries[*i].1)
    }
    pub(crate) fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
    pub(crate) fn capacity(&self) -> usize {
        24 + 64 * self.0.entry_capacity + self.0.byte_capacity
    }
    /// Complete deterministic order; keys break equal-value ties.
    pub(crate) fn sort(&mut self, values_first: bool) {
        let storage = Arc::make_mut(&mut self.0);
        storage.entries.sort_unstable_by(|(ak, av), (bk, bv)| {
            let values = if values_first {
                match (av, bv) {
                    (EntryValue::Integer(a), EntryValue::Integer(b)) => a.cmp(b),
                    (EntryValue::Bytes(a), EntryValue::Bytes(b)) => a.cmp(b),
                    _ => unreachable!("homogeneous validated map"),
                }
            } else {
                std::cmp::Ordering::Equal
            };
            values.then_with(|| ak.cmp(bk))
        });
        storage.index.clear();
        for (i, (key, _)) in storage.entries.iter().enumerate() {
            storage.index.insert(key.clone(), i);
        }
    }
    pub(crate) fn update(
        &mut self,
        key: &[u8],
        value: EntryValue,
        insert: bool,
        live: &mut usize,
        span: Option<Span>,
    ) -> Result<(), Diagnostic> {
        let position = self.0.index.get(key).copied();
        if insert && position.is_some() {
            return Err(fault("E020", span, "duplicate map key"));
        }
        let old_bytes = position.map_or(0, |i| self.0.entries[i].1.bytes_len());
        let used = self.0.used - old_bytes
            + value.bytes_len()
            + if position.is_none() { key.len() } else { 0 };
        let count = self.0.entries.len() + usize::from(position.is_none());
        let mut ec = self.0.entry_capacity;
        let mut bc = self.0.byte_capacity;
        while ec < count {
            ec *= 2;
        }
        while bc < used {
            bc *= 2;
        }
        charge(live, 24 + 64 * ec + bc, span)?;
        let storage = Arc::make_mut(&mut self.0);
        storage.entry_capacity = ec;
        storage.byte_capacity = bc;
        storage.used = used;
        if let Some(i) = position {
            storage.entries[i].1 = value;
        } else {
            storage.index.insert(key.to_vec(), storage.entries.len());
            storage.entries.push((key.to_vec(), value));
        }
        Ok(())
    }
}
