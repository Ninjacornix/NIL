# Milestone 12 — Self-hosting investigation

Status: **Optional late investigation; not a core success criterion**.

## Objective

Decide whether a NIL-written compiler provides enough value to pursue.

## Motivation

Treat self-hosting as a research/business decision rather than an early success criterion.

## Dependencies

M10 and mature language/runtime evidence; NIL-102.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-120 | Inventory compiler capabilities absent from NIL; compare self-hosting benefits with maintenance, bootstrapping and token-efficiency costs. | NIL-102 |
| NIL-121 | If justified, prototype one small compiler component in NIL and define staged bootstrap/trust strategy. | NIL-120 |
| NIL-122 | Write go/no-go ADR with effort, correctness and performance evidence; plan full port only after explicit scope approval. | NIL-121 |

## Tests

Differential component outputs against Rust, deterministic bootstrap artifacts where applicable and stage/version compatibility tests.

## Benchmark requirements

Compiler latency, maintenance effort and generation/repair cost; self-hosting alone is not a token-efficiency result.

## Deliverables

Feasibility report, optional component prototype and explicit go/no-go decision.

## Acceptance criteria

Benefits and costs are measured; no-go is a successful investigation. A full compiler rewrite requires a separate approved plan.

## Explicitly excluded work

Automatic Rust replacement, full port during investigation and sacrificing core simplicity to claim self-hosting.

## Risks and open questions

Feature pressure, bootstrap trust, long-term maintenance and unclear benefit.

## 2026-09-30 audit

Keep this as an optional go/no-go investigation. No current token-efficiency,
semantic correctness or native-performance goal requires a NIL-written compiler.
Do not add compiler-sized features merely to satisfy self-hosting. Rust remains
the implementation language; see the [audit](../MILESTONE_AUDIT.md).
