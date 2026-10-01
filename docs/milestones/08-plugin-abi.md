# Milestone 8 — Minimal plugin ABI

Status: **Planned; not implemented**.

## Objective

Prove one typed semantic extension without parser-specific syntax.

## Motivation

Keep the core small while permitting new instruction sets.

## Dependencies

Stable current primitive types and validated HIR (M2 / NIL-022); explicit plugin identity, effect and lowering contracts. Additional M3 types and serialized M4/MIR only when an operation requires them; M6 desirable for compression measurements.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-080 | Specify registry identity/versioning, operation signatures, effects, runtime requirements and deterministic local alias resolution. | NIL-022; current HIR/type contract |
| NIL-081 | Implement generic plugin-call validation/lowering and one nil-test plugin with primitive input/result types; provide reference host binding and the selected LLVM execution path. | NIL-080 |
| NIL-082 | Verify plugin compatibility and unsupported capability diagnostics; specify and test the selected host ABI/runtime requirements; dynamic loading remains excluded. | NIL-081 |

## Tests

Unknown plugin/op, version mismatch, wrong arguments/results, missing capability/runtime, deterministic registry resolution and successful typed operation execution.

## Benchmark requirements

Measure plugin schema context and generated call tokens separately; compare equal abstractions.

## Deliverables

Registry schema, generic call semantics, tiny test plugin, lowering/execution tests and ABI ADR.

## Acceptance criteria

A new registered test operation executes and returns a validated result with no parser changes; failure cases produce structured errors.

## Explicitly excluded work

Full ecosystem, package manager, HTTP/database plugins, arbitrary parser extensions and dynamic native code loading.

## Risks and open questions

Effects/ownership/ABI lifetime rules remain risks. Parser call delimitation must not depend on loading untrusted plugin code.

## 2026-09-30 audit

No registry, plugin-call HIR operation, typed plugin lowering or plugin execution
exists. The architecture describes a seam, not an implemented ABI. The first tiny
plugin can use i64/bool; arrays, public canonical serialization and a duplicate MIR
are not automatic prerequisites. LLVM already exists, so a backend-free binding
plan is stale. No plugin implementation is authorized by this audit.
