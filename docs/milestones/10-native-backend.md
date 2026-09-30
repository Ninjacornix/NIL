# Milestone 10 — Native execution and optimization

Status: **Partial — user-authorized LLVM native default for the current core**.

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

## Authorized native implementation

The user selected LLVM and explicitly requested compiled default execution after M2.
[ADR 012](../adr/012.md) documents this scope change. nil-llvm now lowers current
validated HIR to LLVM SSA blocks/phi nodes and uses host Clang for AOT build/link.
`nil run` compiles natively; `nil build` saves an executable; no interpreter fallback.
O0/O2 differential tests preserve every current operation, traps, fuel and depth.
[Build/run instructions](../language/NATIVE_LLVM.md) and
[measured performance](../../benchmarks/paired/results/2026-09-30/NATIVE_LLVM.md)
record frontend, IR lowering, LLVM codegen, runtime compilation, linking and size.

This delivers a bounded LLVM portion of NIL-101/NIL-102. NIL-100's multi-backend
comparison, full portable M4 IR, unsupported aggregates/plugins, cross-compilation,
production hardening and complete M10 acceptance remain outstanding. They are not
claimed complete or silently implemented as part of the default-backend switch.
