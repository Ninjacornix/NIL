# Milestone 3 — Evidence-driven types

Status: **Broader types planned; i64/bool HIR typing already implemented**.

## Objective

Extend static types only for concrete acceptance programs.

## Motivation

Preserve a small type system while making later memory and plugin semantics possible.

## Dependencies

M2; NIL-022.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-030 | Create a test-driven type requirements table; decide integer widths/conversions and select the smallest aggregate needed by array-sum/record-projection tests. | NIL-022 |
| NIL-031 | Specify and implement selected types, explicit conversions and aggregate checks; keep unsupported types rejected. | NIL-030 |
| NIL-032 | If tests require references, write ownership/lifetime/bounds policy first; implement only that approved semantic scope and its invalid-program suite. | NIL-031 |

## Tests

Positive and negative signature checks, conversion boundaries, aggregate construction/access, array bounds, layout-independent behavior; memory lifetime tests only if references enter scope.

## Benchmark requirements

Add representative typed tasks; measure validation cost and payload size separately. Do not use compressed type syntax as a token claim.

## Deliverables

Type ADR/spec update, selected type implementations and paired semantic fixtures.

## Acceptance criteria

Every new type has a motivating program and rejection tests; no implicit lossy conversions; evaluator and validator agree.

## Explicitly excluded work

Classes, subtyping, generics, HM inference, templates and speculative float/pointer support.

## Risks and open questions

Exact type set remains open. Floats require NaN/rounding contracts; references require safety and ownership decisions.

## 2026-09-30 audit

M2 already delivered bool intermediates, typed regions and strong static checking;
source function signatures remain i64. NIL-030/031 have not selected or implemented
aggregates, conversions, additional integer widths or floats. NIL-032 is conditional,
not a requirement to add references. New types should answer acceptance programs;
a current-core generation benchmark can proceed without them. See the
[scope audit](../MILESTONE_AUDIT.md).
