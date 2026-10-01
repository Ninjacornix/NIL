# Milestone 2 — Control flow

Status: **Complete — NIL-020, NIL-021 and NIL-022 implemented and verified**.

## Objective

Support comparisons, conditional execution and one looping construct.

## Motivation

Move from expression examples to useful algorithmic tests without adding redundant syntax.

## Dependencies

M1; NIL-014.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-020 | Write control-flow ADR comparing structured regions with explicit CFG; specify bool conditions, scope, joins and loop-carried values before editing IR. | NIL-014 |
| NIL-021 | Extend parser/HIR/checker with chosen comparisons and control flow; update validator and evaluator for branch-local values and loop state. | NIL-020 |
| NIL-022 | Add factorial, iterative Fibonacci, max and counted-sum acceptance programs; update diagnostic and benchmark fixtures. | NIL-021 |

## Tests

Zero/one/many iterations, both branches, nested regions, bad condition types, missing branches/returns, out-of-scope values, loop-carried type mismatches, fuel termination and signed bounds.

## Benchmark requirements

Time frontend and evaluator on bounded loop workloads; retain M1 comparison. Define exact factorial/Fibonacci input domains to avoid overflow ambiguity.

## Deliverables

Control-flow specification and ADR, implemented bool/comparisons/one loop form, four executable examples.

## Acceptance criteria

All four acceptance programs agree with independently calculated reference outputs; rejected control-flow programs cannot reach execution.

## Explicitly excluded work

Multiple loop syntaxes, exceptions, general aggregates, optimizer and native backend.

## Risks and open questions

Bool is an intentional dependency introduced before M3. Structured HIR joins must not force the LLM to spell low-level phi bookkeeping.

## Completion evidence

- NIL-020: [ADR 011](../adr/011.md) chooses structured regions over source CFG,
  explicit bool conditions, branch-local scope, typed joins and simultaneous state updates.
- NIL-021: [control-flow specification](../language/CONTROL_FLOW.md), all expression
  parsers, typed lowering, independent HIR validation and explicit-stack execution.
  expr-v2 uses the measured compact @ marker; expr-v0/v1 use loop. lines-v0 stays
  the compatibility arithmetic profile. Each profile has one loop spelling.
- NIL-022: executable factorial, iterative Fibonacci, max and counted_sum examples
  with checked stdout files; eight paired Python algorithms and 192 reference vectors.

Run `./scripts/ci.sh` and `./scripts/ci.sh release` for formatting, Clippy, builds,
all tests and examples. Tests cover full acceptance domains, both/lazy branches,
nested regions, simultaneous updates, bool state, recursive calls, malformed input,
invalid external HIR, fuel/call limits, signed boundaries and golden diagnostics/IR.

Reproduce token, complete frontend and evaluator measurements:

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/control.py --output /tmp/control-flow.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

[Recorded results](../../benchmarks/reports/2026-09-30/CONTROL_FLOW.md)
include both pinned tokenizers, repeated timings, complete source hashes, and the
M1 comparison rerun with the current interpreter. On the eight-case corpus,
expr-v2 strictly beats Python and expr-v0 per case with both tokenizers. Raw source
counts do not measure generation success or repair/TCR. Runtime speed is independent.

Discovery: a deep hand-built HIR test exposed validator host-stack exhaustion.
HIR/AST regions now have a separately enforced 32-level nesting bound, tested at
32/33 and on deeper external HIR. Expression parsing retains its 128-level limit.
Fibonacci stops one step early and selects its final value to avoid computing F93
when the requested F92 fits. No native backend or optimizer was added.
