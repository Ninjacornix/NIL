<!-- Title: feat(compiler): add compact profiles and control flow
Head: exp/token-reduction
Base: feat/macos-release-binaries
Merge method: Squash and merge
-->

## Purpose

Test whether compact expression representations reduce actual tokenizer counts while sharing the same compiler semantics. Add the control-flow operations needed for factorial, Fibonacci, maximum, and counted loops without requiring source-level phi nodes or mutable locals.

## Changes

- Add opt-in `expr-v1` and `expr-v2` profiles with declaration-order function calls and positional parameters.
- Add bool intermediates, signed comparisons, lazy branches, and typed loop-state tuples with simultaneous updates.
- Extend parsing, type checking, HIR validation, and the bounded reference evaluator for nested regions.
- Add runnable examples, compile-fail cases, HIR/diagnostic goldens, and paired NIL/Python token and runtime reports.

## Validation

```text
Complete-stack checks at ab37194:
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.

Recorded benchmark runs (not rerun while preparing this description):
uv run --project benchmarks/paired --locked python benchmarks/paired/experiment.py --output /tmp/nil-experiment.json — passed recorded checks.
uv run --project benchmarks/paired --locked python benchmarks/paired/control.py --output /tmp/control-flow.json — passed 192 correctness vectors.

Coverage includes malformed syntax, type/arity errors, region scope, bool conditions,
parallel state updates, nested loops, laziness, fuel/depth limits, CLI integration,
and equivalent HIR across profiles. Reviewed control.hir and
control-condition.diag: typed comparison/branch regions and E007 at bytes 2..3.
No finalized portable MIR is introduced.
```

## Issues and compatibility

`expr-v0` remains default; compact profiles are opt-in. All source function parameters/results remain i64. Loops explicitly pass state into their regions; branch-local values cannot escape. Overflow stays checked and reference execution remains bounded. Raw source-token results do not measure model success, repair cost, or TCR.

See [control-flow specification](https://github.com/Ninjacornix/NIL/blob/exp/token-reduction/docs/language/CONTROL_FLOW.md), [compact profiles](https://github.com/Ninjacornix/NIL/blob/exp/token-reduction/docs/language/EXPR_COMPACT.md), [ADR 011](https://github.com/Ninjacornix/NIL/blob/exp/token-reduction/docs/adr/011.md), and [recorded benchmark](https://github.com/Ninjacornix/NIL/blob/exp/token-reduction/benchmarks/paired/results/2026-09-30/CONTROL_FLOW.md). No linked issue.
