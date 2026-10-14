//! Bounded nominal sums with positional payloads (ADR 050).
//!
//! Unlike records, sums may be recursive: a payload slot is one opaque
//! pointer-sized slot, so a self reference needs no static size. Values are
//! immutable and built bottom-up, so no cycle can be formed and reference
//! counting is complete.
//!
//! Variant names are unique across the whole declaration group, so a
//! constructor never needs qualification. Cached slots are validated, never
//! trusted: callers may hand in any registry.
use crate::{Diagnostic, Phase, Span, Type};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SumVariant {
    pub name: String,
    /// Positional, unnamed payload slots. Empty means a nullary variant, which
    /// is represented unboxed and never allocates.
    pub payload: Vec<Type>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SumDefinition {
    pub name: String,
    pub variants: Vec<SumVariant>,
}

pub fn error(span: Option<Span>, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("E023", Phase::Check, span, message)
}

pub fn definition(
    sums: &[SumDefinition],
    ty: Type,
    span: Option<Span>,
) -> Result<&SumDefinition, Diagnostic> {
    let Type::Sum(id) = ty else {
        return Err(error(span, "sum operand required"));
    };
    sums.get(id).ok_or_else(|| error(span, "unknown sum type"))
}

pub fn validate_type(
    sums: &[SumDefinition],
    records: &[crate::RecordDefinition],
    ty: Type,
    span: Option<Span>,
) -> Result<(), Diagnostic> {
    match ty {
        Type::Sum(_) => {
            definition(sums, ty, span)?;
        }
        Type::Record(id, _) | Type::MapRecord(id, _) | Type::RecordBuffer(id, _) => {
            crate::records::validate_type(records, ty, span)?;
            for field in &records[id].fields {
                validate_type(sums, records, field.ty, span)?;
            }
        }
        _ => crate::records::validate_type(records, ty, span)?,
    }
    Ok(())
}

pub fn variant(
    sums: &[SumDefinition],
    ty: Type,
    index: usize,
    span: Option<Span>,
) -> Result<&SumVariant, Diagnostic> {
    definition(sums, ty, span)?
        .variants
        .get(index)
        .ok_or_else(|| error(span, "unknown sum variant"))
}

/// Declared type of one payload slot; projections must still verify the tag.
pub fn payload(
    sums: &[SumDefinition],
    ty: Type,
    index: usize,
    slot: usize,
    span: Option<Span>,
) -> Result<Type, Diagnostic> {
    variant(sums, ty, index, span)?
        .payload
        .get(slot)
        .copied()
        .ok_or_else(|| error(span, "unknown sum payload slot"))
}

/// Resolve a constructor name to `(sum id, variant index)`. Variant names are
/// unique across the group, so this is unambiguous.
pub fn find_variant(sums: &[SumDefinition], name: &str) -> Option<(usize, usize)> {
    sums.iter().enumerate().find_map(|(id, s)| {
        s.variants
            .iter()
            .position(|v| v.name == name)
            .map(|index| (id, index))
    })
}

/// Which sums need heap nodes (ADR 050 addendum A1).
///
/// A sum is boxed when it is **recursive** — reachable from its own payload,
/// directly or through another sum or a record — or when any payload slot
/// holds a boxed sum. The second condition keeps reference counting confined
/// to values reached only through boxed nodes, so an inline aggregate never
/// carries a counted reference.
///
/// Everything else is laid out inline as a tag plus the widest payload, like a
/// record. `Option` of a scalar and every enumeration land there, so `Some(x)`
/// never allocates. This mirrors Swift's `indirect`, decided by the compiler
/// rather than by an annotation.
pub fn boxed(sums: &[SumDefinition], records: &[crate::RecordDefinition]) -> Vec<bool> {
    let mut boxed = vec![false; sums.len()];
    // Recursive sums first: a sum reaching itself must be boxed.
    for (id, slot) in boxed.iter_mut().enumerate() {
        *slot = reaches(sums, records, id, id, &mut vec![false; sums.len()]);
    }
    // Then propagate: a payload holding a boxed sum forces its owner to box.
    loop {
        let mut changed = false;
        for id in 0..sums.len() {
            if boxed[id] {
                continue;
            }
            if sums[id]
                .variants
                .iter()
                .any(|v| v.payload.iter().any(|ty| holds_boxed(records, *ty, &boxed)))
            {
                boxed[id] = true;
                changed = true;
            }
        }
        if !changed {
            return boxed;
        }
    }
}
fn holds_boxed(records: &[crate::RecordDefinition], ty: Type, boxed: &[bool]) -> bool {
    match ty {
        Type::Sum(id) => boxed.get(id).copied().unwrap_or(true),
        Type::Record(id, _) => records
            .get(id)
            .is_some_and(|r| r.fields.iter().any(|f| holds_boxed(records, f.ty, boxed))),
        _ => false,
    }
}
/// Whether `target` is reachable from sum `from`'s payload.
fn reaches(
    sums: &[SumDefinition],
    records: &[crate::RecordDefinition],
    from: usize,
    target: usize,
    seen: &mut Vec<bool>,
) -> bool {
    if seen[from] {
        return false;
    }
    seen[from] = true;
    sums[from].variants.iter().any(|v| {
        v.payload
            .iter()
            .any(|ty| mentions(sums, records, *ty, target, seen))
    })
}
fn mentions(
    sums: &[SumDefinition],
    records: &[crate::RecordDefinition],
    ty: Type,
    target: usize,
    seen: &mut Vec<bool>,
) -> bool {
    match ty {
        Type::Sum(id) => id == target || reaches(sums, records, id, target, seen),
        Type::Record(id, _) => records.get(id).is_some_and(|r| {
            r.fields
                .clone()
                .iter()
                .any(|f| mentions(sums, records, f.ty, target, seen))
        }),
        _ => false,
    }
}

/// Inline layout width in private tooling slots: one tag plus the widest
/// variant payload. Only meaningful when the sum is not boxed.
pub fn inline_slots(sums: &[SumDefinition], id: usize) -> usize {
    1 + sums[id]
        .variants
        .iter()
        .map(|v| v.payload.iter().map(|ty| ty.slots()).sum::<usize>())
        .max()
        .unwrap_or(0)
}

/// A sum is inhabited when some variant's payload slots are all inhabited.
/// Computed as a least fixed point so mutual recursion converges. `sum X(A(X))`
/// is uninhabited: no program can build a value, which is a user error worth
/// naming rather than leaving to confusing downstream type errors.
fn inhabited(sums: &[SumDefinition], records: &[crate::RecordDefinition]) -> Vec<bool> {
    let mut live = vec![false; sums.len()];
    loop {
        let mut changed = false;
        for (id, s) in sums.iter().enumerate() {
            if live[id] {
                continue;
            }
            let any = s.variants.iter().any(|v| {
                v.payload
                    .iter()
                    .all(|ty| slot_inhabited(*ty, &live, records))
            });
            if any {
                live[id] = true;
                changed = true;
            }
        }
        if !changed {
            return live;
        }
    }
}
fn slot_inhabited(ty: Type, live: &[bool], records: &[crate::RecordDefinition]) -> bool {
    match ty {
        Type::Sum(id) => live.get(id).copied().unwrap_or(false),
        Type::Record(id, _) => records
            .get(id)
            .is_some_and(|r| r.fields.iter().all(|f| slot_inhabited(f.ty, live, records))),
        // Every scalar, fixed array, sequence and map has at least one value.
        _ => true,
    }
}

/// Validates the sum group against an already validated record registry.
/// Recursion between sums, in any declaration order, is permitted; records
/// remain acyclic and earlier-only because they are inline aggregates.
pub fn validate_definitions(
    sums: &[SumDefinition],
    records: &[crate::RecordDefinition],
) -> Result<(), Diagnostic> {
    if sums.len() + records.len() > 128 {
        return Err(error(None, "at most 128 record and sum types"));
    }
    let mut names = std::collections::BTreeSet::new();
    let mut variants = std::collections::BTreeSet::new();
    for r in records {
        if !names.insert(r.name.as_str()) {
            return Err(error(None, "invalid or duplicate type definition"));
        }
    }
    for s in sums {
        if s.name.is_empty() || !names.insert(s.name.as_str()) {
            return Err(error(None, "invalid or duplicate type definition"));
        }
        if s.variants.is_empty() || s.variants.len() > 64 {
            return Err(error(None, "sums have 1 to 64 variants"));
        }
        for v in &s.variants {
            // Unique across the whole group so constructors need no qualifier.
            if v.name.is_empty()
                || names.contains(v.name.as_str())
                || !variants.insert(v.name.as_str())
            {
                return Err(error(None, "invalid or duplicate variant name"));
            }
            if v.payload.len() > 64 {
                return Err(error(None, "at most 64 payload slots per variant"));
            }
            let mut slots = 0usize;
            for ty in &v.payload {
                match ty {
                    // A self or forward sum reference is exactly what this ADR
                    // admits; it is one slot and needs no size.
                    Type::Sum(id) => {
                        if *id >= sums.len() {
                            return Err(error(None, "unknown sum type"));
                        }
                    }
                    _ => crate::records::validate_type(records, *ty, None)?,
                }
                if matches!(ty, Type::Array(n) if *n > crate::MAX_ARRAY_LEN) {
                    return Err(error(None, "invalid variant array slot"));
                }
                slots = slots
                    .checked_add(ty.slots())
                    .ok_or_else(|| error(None, "variant layout overflow"))?;
            }
            if slots > 4096 {
                return Err(error(None, "variant layout exceeds limits"));
            }
        }
    }
    let live = inhabited(sums, records);
    if let Some((id, _)) = live.iter().enumerate().find(|(_, ok)| !**ok) {
        return Err(error(
            None,
            format!("sum {} has no finite values", sums[id].name),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sum(name: &str, variants: Vec<(&str, Vec<Type>)>) -> SumDefinition {
        SumDefinition {
            name: name.into(),
            variants: variants
                .into_iter()
                .map(|(n, payload)| SumVariant {
                    name: n.into(),
                    payload,
                })
                .collect(),
        }
    }

    #[test]
    fn only_recursive_sums_and_their_holders_are_boxed() {
        // Option of a scalar and plain enumerations stay inline, so Some(x)
        // never allocates.
        let option = sum("O", vec![("None", vec![]), ("Some", vec![Type::I64])]);
        assert_eq!(boxed(std::slice::from_ref(&option), &[]), vec![false]);
        assert_eq!(inline_slots(std::slice::from_ref(&option), 0), 2);
        let flags = sum("F", vec![("A", vec![]), ("B", vec![]), ("C", vec![])]);
        assert_eq!(boxed(std::slice::from_ref(&flags), &[]), vec![false]);
        assert_eq!(inline_slots(std::slice::from_ref(&flags), 0), 1);
        // A self-recursive list is boxed.
        let list = sum(
            "L",
            vec![("C", vec![Type::I64, Type::Sum(0)]), ("E", vec![])],
        );
        assert_eq!(boxed(std::slice::from_ref(&list), &[]), vec![true]);
        // Mutual recursion boxes both.
        let a = sum("A", vec![("MkA", vec![Type::Sum(1)]), ("EndA", vec![])]);
        let b = sum("B", vec![("MkB", vec![Type::Sum(0)]), ("EndB", vec![])]);
        assert_eq!(boxed(&[a, b], &[]), vec![true, true]);
        // A non-recursive sum whose payload holds a boxed sum is boxed too, so
        // an inline aggregate never carries a counted reference.
        let inner = sum(
            "L",
            vec![("C", vec![Type::I64, Type::Sum(0)]), ("E", vec![])],
        );
        let holder = sum("H", vec![("Wrap", vec![Type::Sum(0)]), ("Nothing", vec![])]);
        assert_eq!(boxed(&[inner, holder], &[]), vec![true, true]);
    }

    #[test]
    fn a_record_field_can_make_a_sum_recursive() {
        let record = crate::RecordDefinition {
            name: "W".into(),
            fields: vec![crate::RecordField {
                name: "t".into(),
                ty: Type::Sum(0),
            }],
        };
        // S(P(W)) where W holds S: recursion runs through the record.
        let s = sum("S", vec![("P", vec![Type::Record(0, 1)]), ("Q", vec![])]);
        assert_eq!(boxed(&[s], std::slice::from_ref(&record)), vec![true]);
    }

    #[test]
    fn a_recursive_sum_with_a_base_variant_is_accepted() {
        let list = sum(
            "List",
            vec![("Cons", vec![Type::I64, Type::Sum(0)]), ("Nil", vec![])],
        );
        validate_definitions(std::slice::from_ref(&list), &[]).unwrap();
        assert_eq!(
            find_variant(std::slice::from_ref(&list), "Cons"),
            Some((0, 0))
        );
        assert_eq!(
            find_variant(std::slice::from_ref(&list), "Nil"),
            Some((0, 1))
        );
        assert_eq!(find_variant(&[list], "Absent"), None);
    }

    #[test]
    fn mutually_recursive_sums_in_either_declaration_order_are_accepted() {
        // A refers forward to B, which refers back to A.
        let a = sum("A", vec![("MkA", vec![Type::Sum(1)]), ("EndA", vec![])]);
        let b = sum("B", vec![("MkB", vec![Type::Sum(0)])]);
        validate_definitions(&[a, b], &[]).unwrap();
    }

    #[test]
    fn a_sum_with_no_finite_value_is_rejected() {
        let x = sum("X", vec![("OnlyX", vec![Type::Sum(0)])]);
        let e = validate_definitions(&[x], &[]).unwrap_err();
        assert_eq!(e.code, "E023");
        assert!(e.message.contains("no finite values"), "{}", e.message);
    }

    #[test]
    fn mutual_recursion_with_no_base_case_anywhere_is_rejected() {
        let a = sum("A", vec![("MkA", vec![Type::Sum(1)])]);
        let b = sum("B", vec![("MkB", vec![Type::Sum(0)])]);
        assert!(validate_definitions(&[a, b], &[]).is_err());
    }

    #[test]
    fn variant_names_are_unique_across_the_whole_group() {
        let a = sum("A", vec![("Same", vec![]), ("OtherA", vec![])]);
        let b = sum("B", vec![("Same", vec![])]);
        let e = validate_definitions(&[a, b], &[]).unwrap_err();
        assert!(e.message.contains("variant name"), "{}", e.message);
    }

    #[test]
    fn type_names_cannot_collide_with_a_record_name() {
        let r = crate::RecordDefinition {
            name: "Shared".into(),
            fields: vec![crate::RecordField {
                name: "x".into(),
                ty: Type::I64,
            }],
        };
        let s = sum("Shared", vec![("V", vec![])]);
        assert!(validate_definitions(&[s], std::slice::from_ref(&r)).is_err());
    }

    #[test]
    fn variant_names_cannot_collide_with_record_constructors() {
        let record = crate::RecordDefinition {
            name: "P".into(),
            fields: vec![crate::RecordField {
                name: "x".into(),
                ty: Type::I64,
            }],
        };
        let sum = sum("S", vec![("P", vec![Type::I64]), ("Q", vec![])]);
        assert!(validate_definitions(&[sum], &[record]).is_err());
    }

    #[test]
    fn empty_and_oversized_variant_lists_are_rejected() {
        assert!(validate_definitions(&[sum("E", vec![])], &[]).is_err());
        let many = (0..65)
            .map(|i| (Box::leak(format!("V{i}").into_boxed_str()) as &str, vec![]))
            .collect::<Vec<_>>();
        assert!(validate_definitions(&[sum("M", many)], &[]).is_err());
    }

    #[test]
    fn an_unknown_payload_sum_id_is_rejected_and_lookups_are_checked() {
        assert!(validate_definitions(&[sum("A", vec![("V", vec![Type::Sum(7)])])], &[]).is_err());
        let list = sum(
            "List",
            vec![("Cons", vec![Type::I64, Type::Sum(0)]), ("Nil", vec![])],
        );
        let r = std::slice::from_ref(&list);
        assert_eq!(payload(r, Type::Sum(0), 0, 1, None).unwrap(), Type::Sum(0));
        assert!(payload(r, Type::Sum(0), 1, 0, None).is_err());
        assert!(payload(r, Type::Sum(0), 9, 0, None).is_err());
        assert!(definition(r, Type::Sum(9), None).is_err());
        assert!(definition(r, Type::I64, None).is_err());
    }
}
