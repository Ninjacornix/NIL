# Round 21 — measurements before generalization

Status: **in progress**, not a completed performance or ownership round.
Main branch `feat/core-generalize` starts at `2e6614e`; benchmarks branch
`exp/core-generalize` starts at `291d3c9`. Neither has an upstream. Nothing is
pushed. No compiler, runtime, CLI, std, frozen corpus, oracle or golden changes.

## Natural qualification is the headline

Benchmarks commit `e9a39c6` freezes 29 byte-exact Round 19 admitted sources from
`6fd268d`, alongside the hashes of the tuned sources. Thirteen differ; sixteen
are unchanged. The 114 unattempted NeetCode problems remain held out. Performance
rewrites of natural sources are forbidden; correctness corrections require an
explicit record. Round 20's 22/150 is a **tuned** count, not yet a natural count.

The existing verification harness passed all **1,070/1,070** checks for these
unmodified sources (reference, native O0/O2, Python and C++):

```sh
python3 benchmarks/corpora/neetcode150/verify.py --verified-only --jobs 1 \
  --candidate-dir "$PWD/benchmarks/core-generalize-study/natural" \
  --output /tmp/nil-round21/natural-correctness.json
# TOTALS {"tasks":29,"verified_nil":29,"checks":1070,"passed_checks":1070}
```

## Exact token attribution (Part A3)

The unchanged kernel cut reproduces **4,348 versus 2,609 cl100k** and
**4,368 versus 2,620 o200k** tokens: NIL costs **66.7% more** than Python. Every
category sums exactly to the **1,739 / 1,748** surplus. The largest positive
cl100k categories are separators/brackets +867 and numeric literals +527;
signatures +203, operators +203 and conditionals +198 follow. Whitespace saves
455 tokens. Helpers are fully counted, not double-counted under an overlapping
category. These are spelling costs, not forecasts of removable costs.

Five benchmark tests verify source hashes, exact token sums, callback signatures,
binding semicolons, quoted loop markers and conditional classification. The raw
per-token ledger stays outside git. Aggregate reports and reproducible commands
are in `benchmarks/core-generalize-study/` and
`benchmarks/reports/2026-10-07/ROUND21_KERNEL_ATTRIBUTION.*`.

## Still required before compiler changes

- Complete natural/tuned qualification and peak RSS measurements on all admitted
  problems and historical controls; peak observed live charge is measured with a
  separate temporary-runtime overlay, never in timing binaries.
- A physical-copy census with future semantic observation tracking. Static last
  use, root counts and memcpy counts alone are not that census: emitted direct
  element reads, aliases, record descendants and address reuse must be covered.
- Forecasts derived from those measurements, then ADR 043 committed before code.
- General ownership and copy hints only if their complete soundness argument holds.
- Configurable limits and native stack safety, then all post-change gates.

The reference evaluator already uses heap-allocated function frames; claiming it
needs a recursive-to-iterative rewrite would be incorrect. Native depth checks
are currently conditional on bounded instrumentation. A larger configured depth
cannot be considered safe without independently handling native stack exhaustion.
The current 64 MiB limit and reserve-as-if-copy rule remain unchanged pending ADR
043. No performance, copy-census or new-limit success is claimed by this document.
