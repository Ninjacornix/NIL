# Milestone 2 — Control flow

Status: **Planned; not implemented**.

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
