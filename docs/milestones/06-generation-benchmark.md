# Milestone 6 — LLM generation benchmark

Status: **Local Ollama pilot runner implemented; pilot interrupted at the user’s request.**

## Objective

Measure whole generate/check/test/repair trajectories under equal budgets.

## Motivation

Determine whether NIL actually improves tokens to correct programs.

## Dependencies

Current-core subset of M5 / NIL-052: frozen equivalent tasks, pinned profile/compiler, provenance and attempt records. Full canonical-IR corpus and broader M3 types are not pilot prerequisites.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-060 | Freeze task splits, hidden oracle, model snapshots, prompt templates, sampling, token/repair limits and noninferiority margin. | NIL-052 (current-core pilot subset) |
| NIL-061 | Implement model adapter, sandboxed execution, timeouts and immutable attempt logging; count actual usage, repairs and failed trials. | NIL-060 |
| NIL-062 | Implement TCR/TTCP/Solve@budget analysis with paired bootstrap intervals; publish raw failures and held-out results. | NIL-061 |

## Tests

Mock model trajectories including zero solves, partial tests, timeouts, parse/type failures, first-pass solves and repairs. Verify metric arithmetic and no double-counted reasoning/feedback.

## Benchmark requirements

All protocol fields; separate raw source tokens, output and repair costs. Include spec/prompt overhead and compare correctness before efficiency.

## Deliverables

Controlled runner, hidden-test harness, trajectory schema/data and reproducible analysis.

## Acceptance criteria

Identical tasks/oracles/budgets run across NIL and baselines; failed trials contribute to TCR; output supports reproducible correctness/efficiency conclusions.

## Explicitly excluded work

Production agents, unbounded external execution, training and cherry-picked success-only reports.

## Risks and open questions

Model access/budget, test contamination, sample size and hidden tests require pre-registration before paid runs.

## 2026-09-30 audit

The metric/protocol document exists; no model adapter, model-generated trajectory
runner, repair accounting implementation or TCR result exists. Seeded fuzzing is
compiler testing, not completion of this milestone. Prepare a small current-core
pilot using existing source/token tools; mock-test zero solves, failures, feedback
and token accounting before paid runs. Freeze arithmetic and execution limits
across representations. Obtain model/access/cost choices before running external
experiments. The full study remains open; see the [audit](../MILESTONE_AUDIT.md).


## 2026-10-01 local pilot

[Runner and preregistered controls](../../benchmarks/generation/README.md) compare
v0/v1/v2 with matched checked-i64 semantics using installed Gemma 3 4B and Qwen
2.5 7B Instruct. It retains raw requests/responses/source and actual provider usage;
failed trials contribute to TCR. Bounded reference and native execution verify
hidden vectors. Stage failures, unknown usage and zero solves remain explicit.
Metric/oracle tests cover failure accounting, repairs, bootstrap pairing, partial
output extraction, parsing, typing, semantic mismatch and fuel exhaustion.

This is a numerical-core exploratory pilot, not completion of the broader M6
study. Python/C/Rust generation baselines, unseen external tasks, grammar-constrained
decoding and large-sample model studies remain open. No syntax winner is selected
until the declared correctness/TCR gates are evaluated on complete paired results.
