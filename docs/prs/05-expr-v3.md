<!-- Title: feat(syntax): add wrapping expr-v3 semantics
Head: exp/expr-v3
Base: feat/llvm-native
Merge method: Squash and merge
-->

## Purpose

Checked overflow and per-instruction resource accounting restrict optimization in the current integer core. Add an opt-in `expr-v3` experiment with deterministic wrapping arithmetic and optional native accounting, then compare it against ordinary and checked C++ using the same call harness.

## Changes

- Reuse the expr-v2 grammar and attach explicit checked/wrapping arithmetic policy to HIR.
- Define wrapping add/subtract/multiply, trapping division by zero, and MIN / -1 returning MIN.
- Default native v3 execution to unbounded mode; expose `--bounded` and `--unbounded` explicitly.
- Lower wrapping operations without undefined-overflow promises and preserve exceptional division behavior.
- Add examples, signed-boundary and O0/O2 tests, C++ baselines, and regular/stress benchmark reports.

## Validation

```text
Complete-stack checks at ab37194:
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.

Recorded v3 validation (before the later fuzzing crate):
./scripts/ci.sh and ./scripts/ci.sh release — each passed 113 tests.
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests — passed 16 tests.
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v3 --output /tmp/v3.json — passed 192 correctness vectors.
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v3 --stress --output /tmp/v3-stress.json — passed 194 vectors.

Coverage includes strict grammar/type checks, explicit HIR policy, signed extrema,
division traps, lazy regions, calls/recursion, bool/parallel loop state,
CLI policy selection, and bounded/unbounded native agreement at O0/O2.
No HIR or diagnostic golden fixtures change. Backend lowering is differential-tested.
```

## Issues and compatibility

`expr-v0` remains default. Identical source bytes intentionally have different overflow behavior under v2 and v3; callers must preserve profile metadata. Unbounded native execution can run indefinitely or exhaust the host stack. Wrapping arithmetic is defined, unlike ordinary C++ signed overflow; timed comparison inputs fit i64. Local performance and source-token results do not establish LLM-generation efficiency.

See [v3 specification](https://github.com/Ninjacornix/NIL/blob/exp/expr-v3/docs/language/EXPR_V3.md), [ADR 013](https://github.com/Ninjacornix/NIL/blob/exp/expr-v3/docs/adr/013.md), and [recorded measurements](https://github.com/Ninjacornix/NIL/blob/exp/expr-v3/benchmarks/paired/results/2026-09-30/EXPR_V3.md). No linked issue.
