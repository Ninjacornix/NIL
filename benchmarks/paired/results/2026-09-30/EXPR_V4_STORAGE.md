# expr-v4: compact source and private loop storage

The final local macOS/Apple Silicon run passes the per-kernel NIL/C++ ≤1.25 gate
for all 25 original kernels and nine additional kernels. The worst ratio is 1.118;
the additional corpus's worst is 1.023. These are measured numerical kernels,
not a universal guarantee about arbitrary programs or hardware.

[Primary raw results](expr-v4-storage.json), [additional raw results](expr-v4-storage-heldout.json),
and [frontend/checker/payload results](expr-v4-storage-validation.json) preserve
settings, tool versions, input fixtures, source hashes, samples, compile stages
and binary sizes. All recorded source hashes match the final tested working tree.
The [historical report](EXPR_V4.md) remains available for the old copying baseline.

## Change and semantics

[ADR 015](../../../../docs/adr/015.md) records syntax-independent def-use analysis:
proven single-use replacement chains, including conditional updates, use separate
private loop buffers. Bounds checks and value evaluation stay at their original
positions; writes commit after body evaluation. Unsupported/escaping chains retain
aggregate copying. Aliases, old reads, lazy branches, simultaneous updates and
bounded-accounting traps retain their semantics. Typed functions are internal
behind the entry bridge and carry an inlining hint. No source token, type or
observable array-value semantics changed.

## Token results

Complete program source, excluding shared entry/timing adapters:

| Corpus | NIL cl100k / Qwen | Python cl100k / Qwen | C++ cl100k / Qwen |
|---|---:|---:|---:|
| Original 25 | 851 / 906 | 1068 / 1068 | 3815 / 4112 |
| Additional 9 | 264 / 288 | 357 / 357 | 1329 / 1428 |
| Combined 34 | 1115 / 1194 | 1425 / 1425 | 5144 / 5540 |

Combined NIL uses 21.8%/16.2% fewer tokens than Python. C++ complete-source counts
include headers and checked-read helpers, so the difference is not purely algorithm
size; available function-only counts are recorded separately. This measures actual
pinned tokenizer output, not characters. Small unrolled sums can still beat array
loops in source tokens. Model-generated validity, repairs and TCR remain unmeasured.

## Native runtime

Nanoseconds per call; ratio uses medians. All kernels pass the 1.25 gate.

| Kernel | NIL O2 | C++ O2 | NIL / C++ |
|---|---:|---:|---:|
| sum-8 | 0.907 | 0.903 | 1.004 |
| dot-8 | 2.307 | 2.321 | 0.994 |
| max-8 | 1.064 | 1.039 | 1.024 |
| search-8 | 3.350 | 3.253 | 1.030 |
| reverse-8 | 1.799 | 1.760 | 1.022 |
| prefix-8 | 2.047 | 2.039 | 1.004 |
| sum-32 | 1.919 | 1.904 | 1.008 |
| dot-32 | 11.487 | 11.375 | 1.010 |
| max-32 | 3.249 | 3.277 | 0.991 |
| search-32 | 7.012 | 6.978 | 1.005 |
| reverse-32 | 8.142 | 8.017 | 1.016 |
| prefix-32 | 9.832 | 9.991 | 0.984 |
| sum-128 | 7.006 | 6.961 | 1.006 |
| dot-128 | 57.874 | 57.818 | 1.001 |
| max-128 | 28.574 | 27.990 | 1.021 |
| search-128 | 19.376 | 19.321 | 1.003 |
| reverse-128 | 65.551 | 64.221 | 1.021 |
| prefix-128 | 91.080 | 91.790 | 0.992 |
| sum-256 | 37.496 | 37.576 | 0.998 |
| dot-256 | 137.535 | 123.071 | 1.118 |
| max-256 | 62.094 | 62.192 | 0.998 |
| search-256 | 39.129 | 39.155 | 0.999 |
| reverse-256 | 147.365 | 139.268 | 1.058 |
| prefix-256 | 208.268 | 195.243 | 1.067 |
| predicate | 0.897 | 0.937 | 0.957 |
| map-16 | 5.786 | 5.804 | 0.997 |
| conditional-16 | 4.760 | 4.876 | 0.976 |
| swap_pairs-16 | 4.842 | 4.731 | 1.023 |
| map-64 | 34.289 | 34.344 | 0.998 |
| conditional-64 | 52.680 | 52.053 | 1.012 |
| swap_pairs-64 | 17.677 | 17.663 | 1.001 |
| map-192 | 106.516 | 106.984 | 0.996 |
| conditional-192 | 169.268 | 166.183 | 1.019 |
| swap_pairs-192 | 85.300 | 85.417 | 0.999 |

Length-256 reverse/prefix now measure 147.365/208.268 ns versus C++
139.268/195.243 ns: 5.8%/6.7% slower. Historical 147×/105× ratios described the
previous aggregate-copy lowering. Both versions retain identical NIL source.

## Method and validation

Nine interleaved repeats, one million optimized native calls per sample, 1000-call
warmup, identical generated C timing driver, flat entry bridges, separate translation
units and no LTO. C++ uses O2 and `-fwrapv`; transforms use faster fresh-local mutation
rather than forced per-element value copies. Every fixture is checked against an
independent expected result in NIL O0/O2, C++ and Python: 424 comparisons, 68 NIL
builds and 34 C++ builds. Additional map/conditional/swap-pair kernels exercise
sizes 16/64/192 outside the original corpus; they informed development and are not
an untouched statistical holdout. Timed inputs are the first fixture per kernel.

Measurements exclude process startup, compilation and input parsing, but include
marshaling, result comparison and calls. Python includes its result comparison.
Build-stage timings are individual samples. Tiny runtimes remain sensitive to
noise; no confidence interval or universal performance conclusion is claimed.
Checked index semantics remain mandatory. Numeric domains avoid Python/C++ overflow
and division differences. Unbounded native accounting is the timed configuration.

Debug and release suites each pass 148 Rust tests. Formatting, Clippy with warnings
denied, all-target builds, Rust 1.85 checking and 19 Python tests pass. Regression
coverage includes parser/type failures, independent HIR validation, lowering,
execution, aliases, nested/conditional chains, inactive invalid indices, old-value
reads, repeated writes and exact bounded trap behavior at O0/O2. Seeded update
chains match independent wrapping oracles. Linux was not measured locally.

## Reproduce

```sh
./scripts/ci.sh
./scripts/ci.sh release
cargo +1.85.0 check --workspace --all-targets --locked --offline
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/paired/tests
benchmarks/paired/.venv/bin/python benchmarks/paired/typed.py --baseline local --output benchmarks/paired/results/2026-09-30/expr-v4-storage.json --iterations 1000000 --o0-iterations 2000 --python-iterations 2000 --repeats 9
benchmarks/paired/.venv/bin/python benchmarks/paired/typed.py --manifest benchmarks/paired/typed/heldout.json --baseline local --output benchmarks/paired/results/2026-09-30/expr-v4-storage-heldout.json --iterations 1000000 --o0-iterations 2000 --python-iterations 2000 --repeats 9
benchmarks/paired/.venv/bin/python benchmarks/paired/typed_check.py --output benchmarks/paired/results/2026-09-30/expr-v4-storage-validation.json
```

Tokenizer setup is documented in the paired benchmark README. Results identify
both exact tokenizer revisions and the dirty Git revision plus source hashes;
these local changes have not been committed or pushed.
