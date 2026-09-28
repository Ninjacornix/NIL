# Milestone 4 — Canonical semantic IR

Status: **Planned; not implemented**.

## Objective

Formalize deterministic HIR normalization and a validated lowered MIR.

## Motivation

Make representation experiments and backend differential tests share stable semantic contracts.

## Dependencies

M2 and the selected M3 type contract; NIL-022, NIL-031.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-040 | Specify HIR normalization boundaries and versioned canonical serialization, including function/value ordering and metadata exclusion. | NIL-031 |
| NIL-041 | Define MIR basic blocks, SSA block arguments, terminators and effects; implement deterministic HIR→MIR lowering plus dominance/type validation. | NIL-040 |
| NIL-042 | Add round-trip/idempotence goldens and differential HIR/MIR execution or equivalent test oracle. Publish format compatibility rules. | NIL-041 |

## Tests

Canonicalization idempotence; serialization round-trip; alpha-renaming and whitespace convergence; malformed CFGs, bad block arguments/dominance; differential branching/loop results.

## Benchmark requirements

Record normalization/lowering/validation phases separately and serialized bytes; do not claim universal semantic equivalence normalization.

## Deliverables

IR invariant specification, canonical format, MIR implementation and validators, regression corpus.

## Acceptance criteria

Equivalent supported structural spellings normalize identically; every emitted MIR validates and preserves tested HIR behavior.

## Explicitly excluded work

Global optimization, universal program-equivalence checking, final plugin ABI and multiple backends.

## Risks and open questions

Effect order and trapping arithmetic prevent arbitrary reordering. Long-term serialized format stability requires explicit versioning.
