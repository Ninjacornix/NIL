# Milestone 4 — Canonical semantic IR

Status: **Partial: validated semantic HIR and backend SSA exist; canonical serialization absent; portable MIR conditional**.

## Objective

Formalize versioned structural HIR serialization; add a portable MIR only for a concrete consumer or compiler requirement.

## Motivation

Make representation experiments and backend differential tests share stable semantic contracts.

## Dependencies

M2 / NIL-022 and a frozen current-core type/arithmetic contract. Selected M3 additions require format versioning; they do not block current-core serialization.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-040 | Reuse documented invariants; specify normalization boundaries and versioned canonical serialization, including arithmetic policy, function/value ordering and metadata exclusion. | NIL-022; frozen core contract |
| NIL-041 | Conditional: define portable MIR blocks, arguments, terminators and effects only for a named consumer; implement lowering and dominance/type validation. | NIL-040; concrete consumer |
| NIL-042 | Add HIR serialization round-trip/idempotence goldens and compatibility rules; add MIR validation/differential tests if NIL-041 is selected. | NIL-040; NIL-041 only for MIR |

## Tests

Canonicalization idempotence; serialization round-trip; alpha-renaming and whitespace convergence; malformed CFGs, bad block arguments/dominance; differential branching/loop results.

## Benchmark requirements

Record normalization/lowering/validation phases separately and serialized bytes; do not claim universal semantic equivalence normalization.

## Deliverables

IR invariant specification, canonical HIR format and regression corpus; MIR implementation/validators only if selected.

## Acceptance criteria

Equivalent supported structural spellings normalize identically; every emitted MIR validates and preserves tested HIR behavior.

## Explicitly excluded work

Global optimization, universal program-equivalence checking, final plugin ABI and multiple backends.

## Risks and open questions

Effect order and trapping arithmetic prevent arbitrary reordering. Long-term serialized format stability requires explicit versioning.

## 2026-09-30 audit

Immutable typed HIR, region invariants, independent validation, deterministic IDs,
cross-profile equivalence tests and LLVM SSA/phi lowering are implemented. Debug
HIR output is not canonical serialization. Serialization/round-trip work remains;
LLVM's internal block builder is not a public portable MIR. A current-core M6
pilot and the existing native backend do not require another IR. Conditional MIR
acceptance/tests apply only if NIL-041 is selected; serialize HIR first. See the
[audit](../MILESTONE_AUDIT.md).
