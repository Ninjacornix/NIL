# Round 26 — data types checkpoint

**Nothing is implemented yet. ADR 050 is preregistered; B1–B4 and the
natural-v2 cohort are unstarted.**

Branches: `feat/core-types` from `origin/master` 7af71c6 (Round 25 R2 merged);
submodule `exp/core-types` from 098f8f4. Nothing pushed.

[ADR 050](adr/050.md) settles the design and publishes forecasts. Its spelling
tables are measured with tiktoken 0.12.0 on real program text, not fragments.

The measurement that motivates the round, on the frozen kernel cut of
`balanced-binary-tree` (cl100k): NIL v1 292 tokens, **+78.0% vs Python's 164**;
the v2 draft using sum types, recursive types and pattern matching is **107
tokens, −34.8% vs Python**. The token deficit on algorithmic code is the
integer-link/offset-math workaround, not the surface syntax.

Chosen spellings, each within one token of its alternatives so ties broke on
ambiguity and consistency: `sum Name(Variant(types),Nullary)`, prefix
`?scrutinee(Variant(binds):expr,...)`, built-in monomorphized `Option`, `v[v]`
for nested sequences. New Check diagnostics E025 (non-exhaustive match) and
E026 (unreachable arm); codes through E024 were already allocated.

## Status by step

| Step | State |
|---|---|
| ADR 050 | committed before code |
| B1 sums + recursive + match | not started |
| B2 Option | not started |
| B3 nested containers | not started |
| B4 record-buffer E014 | diagnosed: implementation already returns E014; EXPR_V5 documents E013 wrongly, so B4 is a doc fix plus an O0/O2 regression test |
| natural-v2 cohort | not started |
