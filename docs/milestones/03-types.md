# Milestone 3 — Evidence-driven types

Status: **Complete for the selected expr-v4 experiment; bulk-update performance gap documented.**

## Objective and motivation

Extend expr-v3 with the smallest strongly typed aggregate needed for real array
algorithms and bool function boundaries. Preserve existing profiles and their default.
Dependencies: M2, NIL-022. The implementation remains opt-in, not a final syntax choice.

## Type requirements and decisions

| Acceptance program | Required capability | Decision |
|---|---|---|
| Sum, dot product, maximum, binary search | Aggregate parameters, integer indexing and static length | Fixed-length i64 arrays, lengths 0..256 |
| Reverse, prefix sums | Aggregate construction, updates, loop state and returns | Immutable value replacement; simultaneous updates preserve aliases |
| Predicate and typed helper calls | Bool parameters/results | Explicit `b` signature; no integer truthiness |
| Boundary arithmetic | Deterministic integer semantics | Retain wrapping i64 and defined division from v3 |
| Drivers, arbitrary buffers, floating point | Memory layout, references, extra types | Separate requirements; no motivating acceptance program in this corpus |

## Implementation tasks

| ID | Work | Depends on | Status |
|---|---|---|---|
| NIL-030 | Select types from acceptance programs; document widths/conversions, aggregate contract and competing signature encodings. | NIL-022 | Delivered: ADR 014 and expr-v4 spec |
| NIL-031 | Implement selected types, typed signatures, construction/access/replacement, validation, reference/native execution and equivalent fixtures. | NIL-030 | Delivered: compiler/native tests, property tests and paired reports |
| NIL-032 | If references become necessary, specify ownership/lifetime/bounds before implementation. | NIL-031 | Not triggered; no references in this scope |

## Tests

Positive/negative signatures; array-length mismatch; malformed and oversized arrays;
wrong element/index types; empty arrays; lazy traps; immutable aliases; simultaneous
loop state; forward/recursive typed calls; CLI flattening and bool encoding. Review
HIR/diagnostic goldens, independently computed seeded array results and source/HIR
mutations. Compare native O0/O2, bounded/unbounded, with reference execution.
Every earlier profile retains its tests; existing v3 examples have identical v4 HIR.

## Benchmark requirements and deliverables

`benchmarks/paired/typed.py` measures 25 paired programs, lengths 8/32/128/256:
sum, dot, max, binary search, reverse, prefix sums and a bool-returning call.
Measure independent HIR validation latency and flat payload bytes separately.
Count complete source characters/bytes and actual pinned tokenizer tokens; report
C++ function-only counts separately from required headers/helpers. Compare the
same generated C driver and flat ABI at O0/O2 against C++ O2 and Python. Record
frontend/backend stages, runtime samples, binary sizes and source hashes.
Signature screens and raw tokens do not establish model-generation/TCR superiority.

## Acceptance criteria

All selected types have motivating programs and rejection tests. No implicit lossy
conversion; evaluator, validator and native execution agree. Equivalent Python/C++
programs pass independent oracle checks; reproducible token/speed results and
limitations are recorded. Preserve backward compatibility and the expr-v0 default.

## Explicitly excluded work

Extra integer widths, floats, nested/heterogeneous aggregates, dynamic arrays,
references/pointers, explicit allocation, ABI layout, globals, classes, generics,
subtyping and inference. Future type additions need their own examples/contracts.

## Risks and open questions

Value replacement may copy; measure lowering and scaling before changing semantics.
Fixed lengths/capacity and numeric signatures are experimental. Model success and
repair cost remain unmeasured; systems memory needs a separate ownership/effect
contract. The array CLI bridge is tooling, not a stable external ABI.

## Delivery evidence

[Specification](../language/EXPR_V4.md), [ADR 014](../adr/014.md), and
[measured results](../../benchmarks/paired/results/2026-09-30/EXPR_V4.md) record the
selected scope and tradeoffs. Debug/release suites pass 148 Rust tests; 19 benchmark
tests pass; Rust 1.85 checks all targets. Paired runs verify 79 fixtures across four
implementations, plus fresh-local variants. Numeric lengths reduce the tested
signature tokens, and [ADR 015](../adr/015.md) removes repeated copying for proven sparse
loop updates while retaining immutable semantics. The storage report remeasures
fresh-local baselines under a per-kernel speed gate. This closes scoped type
implementation/measurement, not model/TCR selection or general performance claims. References (NIL-032) remain untriggered.
