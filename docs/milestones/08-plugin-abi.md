# Milestone 8 — Minimal plugin ABI

Status: **Planned; not implemented**.

## Objective

Prove one typed semantic extension without parser-specific syntax.

## Motivation

Keep the core small while permitting new instruction sets.

## Dependencies

M3/M4; benchmark-ready measurement from M6 desirable; NIL-031, NIL-042.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-080 | Specify registry identity/versioning, operation signatures, effects, runtime requirements and deterministic local alias resolution. | NIL-031, NIL-042 |
| NIL-081 | Implement generic plugin-call validation/lowering and one nil-test plugin with primitive input/result types; choose interpreter host binding first. | NIL-080 |
| NIL-082 | Verify plugin compatibility and unsupported capability diagnostics; document native ABI candidate separately until a backend exists. | NIL-081 |

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
