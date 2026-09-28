# Milestone 6 — LLM generation benchmark

Status: **Planned; not implemented**.

## Objective

Measure whole generate/check/test/repair trajectories under equal budgets.

## Motivation

Determine whether NIL actually improves tokens to correct programs.

## Dependencies

M5; NIL-052.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-060 | Freeze task splits, hidden oracle, model snapshots, prompt templates, sampling, token/repair limits and noninferiority margin. | NIL-052 |
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
