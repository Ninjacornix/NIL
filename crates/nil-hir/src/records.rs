//! Bounded nominal products. Cached slots are validated, never trusted.
use crate::{Diagnostic, Phase, Span, Type};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordField {
    pub name: String,
    pub ty: Type,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordDefinition {
    pub name: String,
    pub fields: Vec<RecordField>,
}
pub fn error(span: Option<Span>, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E023", Phase::Check, span, message)
}
pub fn definition(
    records: &[RecordDefinition],
    ty: Type,
    span: Option<Span>,
) -> Result<&RecordDefinition, Diagnostic> {
    let Type::Record(id, slots) = ty else {
        return Err(error(span, "record operand required"));
    };
    let r = records
        .get(id)
        .ok_or_else(|| error(span, "unknown record type"))?;
    if r.fields
        .iter()
        .try_fold(0usize, |sum, f| sum.checked_add(f.ty.slots()))
        != Some(slots)
    {
        return Err(error(span, "record slot layout mismatch"));
    }
    Ok(r)
}
pub fn field(
    records: &[RecordDefinition],
    ty: Type,
    index: usize,
    span: Option<Span>,
) -> Result<Type, Diagnostic> {
    definition(records, ty, span)?
        .fields
        .get(index)
        .map(|f| f.ty)
        .ok_or_else(|| error(span, "unknown record field"))
}
/// Requires a validated, acyclic registry and a type from that registry.
pub fn has_dynamic(records: &[RecordDefinition], ty: Type) -> bool {
    ty.is_dynamic()
        || match ty {
            Type::Record(id, _) => records[id]
                .fields
                .iter()
                .any(|f| has_dynamic(records, f.ty)),
            _ => false,
        }
}
pub fn validate_type(
    records: &[RecordDefinition],
    ty: Type,
    span: Option<Span>,
) -> Result<(), Diagnostic> {
    match ty {
        Type::Record(..) => {
            definition(records, ty, span)?;
        }
        Type::MapRecord(id, slots) => {
            definition(records, Type::Record(id, slots), span)?;
            if has_dynamic(records, Type::Record(id, slots)) {
                return Err(error(
                    span,
                    "record map values must have only scalar or fixed-array leaves",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn validate_definitions(records: &[RecordDefinition]) -> Result<(), Diagnostic> {
    if records.len() > 128 {
        return Err(error(None, "at most 128 record types"));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut depths = Vec::new();
    for (id, r) in records.iter().enumerate() {
        if r.name.is_empty() || !names.insert(&r.name) || r.fields.is_empty() || r.fields.len() > 64
        {
            return Err(error(None, "invalid or duplicate record definition"));
        }
        let mut fields = std::collections::BTreeSet::new();
        let mut slots = 0usize;
        let mut depth = 1;
        for f in &r.fields {
            if f.name.is_empty() || !fields.insert(&f.name) {
                return Err(error(None, "invalid or duplicate field"));
            }
            if let Type::Record(child, _) | Type::MapRecord(child, _) = f.ty {
                if child >= id {
                    return Err(error(None, "records refer only to earlier definitions"));
                }
                depth = depth.max(depths[child] + 1);
            }
            validate_type(&records[..id], f.ty, None)?;
            if matches!(f.ty, Type::Array(n) if n > crate::MAX_ARRAY_LEN) {
                return Err(error(None, "invalid record array field"));
            }
            slots = slots
                .checked_add(f.ty.slots())
                .ok_or_else(|| error(None, "record layout overflow"))?;
        }
        if slots > 4096 || depth > 32 {
            return Err(error(None, "record layout exceeds limits"));
        }
        depths.push(depth);
    }
    Ok(())
}
