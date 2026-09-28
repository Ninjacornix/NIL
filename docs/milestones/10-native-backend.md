# Milestone 10 — Native execution and optimization

Status: **Planned; not implemented**.

## Objective

Choose an evidence-backed native/portable backend and preserve reference semantics.

## Motivation

Separate fast iteration from production performance only if measurements justify it.

## Dependencies

M4; plugin backend support depends on M8; NIL-042, optionally NIL-082.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-100 | Benchmark bounded Cranelift/LLVM/WASM prototypes against compile-latency, runtime, size and maintenance criteria; update ADR-002 before production integration. | NIL-042 |
| NIL-101 | Implement one selected backend for the validated core; add required ABI/link/runtime handling and deterministic errors. | NIL-100 |
| NIL-102 | Add interpreter/native differential corpus and measure optimization tradeoffs; support a second backend only with demonstrated need. | NIL-101 |

## Tests

Differential arithmetic/traps, signed division, calls, control flow and aggregates; plugin ABI tests if supported; reproducible artifact/link tests.

## Benchmark requirements

Frontend, lowering/backend, linking, runtime and binary size separately. Compare debug/development and optimized modes on pinned hardware/toolchains.

## Deliverables

Backend decision, one production path, reproducible build/run commands and performance report.

## Acceptance criteria

All supported reference cases agree with compiled execution, including traps; declared performance targets met or gaps documented before release.

## Explicitly excluded work

Advanced optimization passes without evidence, simultaneous production backends and abandoned interpreter oracle.

## Risks and open questions

Backend platform coverage, deterministic arithmetic traps, toolchain packaging and MLIR integration cost.
