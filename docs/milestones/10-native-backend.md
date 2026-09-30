# Milestone 10 — Native execution and optimization

Status: **Delivered for the current core; production hardening and additional backends remain conditional**.

## Objective

Maintain the selected LLVM native path and reference agreement; evaluate portability or another backend only for demonstrated needs.

## Motivation

Separate fast iteration from production performance only if measurements justify it.

## Dependencies

Current validated HIR, already implemented. New type/plugin support depends on its contract; portable MIR is not a requirement for existing LLVM execution.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-100 | Conditional: compare another backend only for a measured latency, portability or deployment need; LLVM selection is recorded in ADR 012. | Concrete requirement; supported HIR contract |
| NIL-101 | Delivered: selected LLVM backend, host ABI/link/runtime and deterministic errors. Extend only for selected new language features. | Validated HIR; ADR 012 |
| NIL-102 | Delivered for current core: interpreter/native O0/O2 differential corpus and separate timing/size measurements. Continue with each new operation. | NIL-101 |

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

NIL-101/NIL-102 are delivered for the supported integer/control-flow core, including
optional accounting and v3 wrapping semantics. Additional backend comparisons,
portable MIR, future aggregate/plugin lowering, cross-target generated code and
production hardening are separate conditional work, not blockers for this scope.
The current-core result does not claim production maturity or universal speed.

## 2026-09-30 audit

The current supported core has native build/run/IR emission, checked/wrapping
arithmetic, optional budgets, O0/O2 reference agreement, host CI, local performance
reports and macOS compiler packaging. This is completion of current-core execution,
not a production/sandboxing or universal performance claim. NIL-100's mandatory
multi-backend prototypes are superseded as an immediate gate by ADR 012. Unsupported types/plugins,
portable MIR, cross-target generated code and production hardening remain separate
work. No custom optimizer or second backend is required now. See the
[audit](../MILESTONE_AUDIT.md).
