//! Owned homogeneous maps. The reference index is deliberately independent of
//! native hashing; insertion order is the shared contract.
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
    pub(crate) fn directory(
        mut names: Vec<Vec<u8>>,
        limit: usize,
        span: Option<Span>,
    ) -> Result<Self, Diagnostic> {
        names.sort();
        // Duplicate names from an injected Host are not a valid directory snapshot.
        if names.windows(2).any(|w| w[0] == w[1]) {
            return Err(fault("E020", span, "duplicate directory name"));
        }
        let mut map = Self::empty(false);
        for name in names {
            let mut scratch = crate::application::Memory::new(limit);
            scratch.bytes = map.capacity() + 40;
            map.update(&name, EntryValue::Integer(0), true, &mut scratch, span)?;
        }
        Ok(map)
    }
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
    /// An integer-keyed set (ADR 051). Keys are the eight big-endian bytes of
    /// the i64, which is injective, so membership is exact across the whole
    /// range including i64::MIN and negatives. Insertion order remains the
    /// contract; the encoding is not an ordering claim.
    pub fn empty_set() -> Self {
        let mut value = Self::empty(false);
        Arc::make_mut(&mut value.0).ty = Type::SetI64;
        value
    }
    pub fn empty_int_map() -> Self {
        let mut value = Self::empty(false);
        Arc::make_mut(&mut value.0).ty = Type::IntMap;
        value
    }
    /// Integer key encoding shared by the reference and native paths.
    pub fn integer_key(key: i64) -> [u8; 8] {
        key.to_be_bytes()
    }
    /// Drops a key if present, reporting whether it was. Insertion order of the
    /// surviving entries is preserved, so iteration stays deterministic.
    pub(crate) fn remove(
        &mut self,
        key: &[u8],
        live: &mut crate::application::Memory,
        span: Option<Span>,
    ) -> Result<bool, Diagnostic> {
        let Some(position) = self.0.index.get(key).copied() else {
            return Ok(false);
        };
        if !self.is_unique() {
            charge(live, self.capacity(), span)?;
        }
        let storage = Arc::make_mut(&mut self.0);
        let (removed, value) = storage.entries.remove(position);
        storage.used -= removed.len() + value.bytes_len();
        storage.index.remove(key);
        for slot in storage.index.values_mut() {
            if *slot > position {
                *slot -= 1;
            }
        }
        Ok(true)
    }
    pub fn empty_record(ty: Type) -> Self {
        let mut value = Self::empty(true);
        Arc::make_mut(&mut value.0).ty = ty;
        value
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
    pub(crate) fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
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
        live: &mut crate::application::Memory,
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
            ec = ec
                .checked_mul(2)
                .ok_or_else(|| fault("E013", span, "map capacity overflow"))?;
        }
        while bc < used {
            bc = bc
                .checked_mul(2)
                .ok_or_else(|| fault("E013", span, "map capacity overflow"))?;
        }
        let capacity = ec
            .checked_mul(64)
            .and_then(|n| n.checked_add(bc))
            .and_then(|n| n.checked_add(24))
            .ok_or_else(|| fault("E013", span, "map capacity overflow"))?;
        if self.is_unique() {
            live.grow(capacity - self.capacity(), span)?;
        } else {
            charge(live, capacity, span)?;
        }
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
