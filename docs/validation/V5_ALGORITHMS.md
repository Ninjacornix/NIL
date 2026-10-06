# expr-v5 sort/each validation

## Decision, compatibility and forecast

[ADR 025](../adr/025.md) is committed as `c7048ee` **before** implementation
`7917834`. Benchmark `4864396` freezes prototypes against `920e510`; sort-only
references were verified before `6d36b83`, then each rewrites before `81e602e`.
Actual savings match the forecasts: sort 299/208/205/203; each marginal
254/193/193/185; joint 553/401/398/388 (Gemma/Qwen/cl100k/o200k). No shortfall.
Standalone each's smaller forecast is not added to the overlapping sort forecast.
Comparison/classification were ranked and rejected. No existing valid program's
meaning changes; earlier profiles/default remain unchanged. No workspace dependency,
unsafe Rust, quota increase or map insertion ceiling change.

[Measured density, all three cuts, per-task counts and attribution movement](../../benchmarks/reports/2026-10-03/V5_ALGORITHMS.md).
External cl100k remains **+252 tokens / +5.00% vs Python**, down from +650/+12.91%.
Source density is not the thesis; generation still measured NIL 0/24 vs Python 11/24.

## CI, diagnostics and memory safety

| Command | Exit | Output (sum of test-result lines) |
|---|---:|---|
| `./scripts/ci.sh` | 0 | 256 passed; no warnings; strict Clippy |
| `./scripts/ci.sh release` | 0 | 256 passed; no warnings |
| `./scripts/sanitize.sh` | 0 | application 37 + keyed 32 = 69 passed |

[Machine-readable receipt and raw log hashes](../../benchmarks/reports/2026-10-03/V5_ALGORITHMS_VALIDATION.json).
Raw logs live under `/tmp/nil-algorithms/`; regenerate with these commands.
Both ASan/UBSan suites run reference/native O0/O2 checks. LeakSanitizer remains
disabled on macOS; this is not a leak audit. No sanitizer finding or differential
compiler failure was found. Tests include signed extrema, unsigned binary/prefix
ordering, deterministic value/key ties, lookup after sorting, retained caller/local
aliases, empty/single inputs, invalid sort mode before result quota, map-sort
capacity quota, each snapshot updates, parallel state, nested/lazy regions,
ignored key/value quota, called effects and exact evaluation order.

**Found and fixed during development:** initial CI failed the existing byte-span
diagnostic golden because binding remapping changed E005 wording. Restored the
existing message; the golden stayed unchanged. The generator's well-typed check
also caught mistaken helper labels; the generator was corrected before measured
runs. This was not a runtime divergence. These failures are reported rather than
calling development clean. All final checks below passed.

IR tests `sort_ir_uses_unique_storage_only_when_last_use_is_proven` assert
`nil_sort_unique` for dead operands and `nil_sort` when old reads remain.
`each_append_state_keeps_unique_proof_while_snapshot_aliases_copy` asserts
`nil_concat_unique`, with snapshot/update aliases independently checked at O0/O2.
Structured iteration lowers to validated canonical HIR Loop, not a new runtime
loop engine; following source-value bindings and earlier diagnostic goldens pass.

## Three seeded differential runs

```sh
for seed in 5130572 1729 8675309; do
  ./scripts/fuzz-v5.sh --seed "$seed" --cases 336 --out "/tmp/nil-algorithms/fuzz-$seed"
done
```

Each run reports `programs:336 native_builds:672 divergences:0`.
**1008 programs  / 2016 O0/O2 builds, zero divergences.**
[Complete per-seed code, intrinsic, call-family and mutation counts](../../benchmarks/reports/2026-10-03/V5_ALGORITHMS_FUZZ.json).
Each named call_family runs twice per seed; all 34 new families are exercised,
204 new-operation cases across the three seeds. They include binary sorting,
value/key ties, signed extrema, aliases/callers/lazy regions/loop state, rebuild
lookup, duplicate-before-sort-mode, snapshot replacement/update, parallel state,
nested loops, called effects, sorted map order, copied byte values, ignored-binding
quota, append alias retention and inherited trap priority. Ordered map results,
stdout and file effects are compared, not just unordered membership.

| Outcome | Per seed | All three |
|---|---:|---:|
| E012 |26|78|
| E013 |24|72|
| E014 |10|30|
| E015 |6|18|
| E016 |18|54|
| E017 |6|18|
| E018 |6|18|
| E019 |2|6|
| E020 |8|24|
| OK |230|690|

Five each source-mutation families each run 36 times per seed (540 total), checking
deterministic diagnostics, spans, accepted HIR validation and bounded execution:
missing separator, truncated expression, unknown operation, extra delimiter and
changed binding. Seed corpus includes sort aliases/binary/quota and each snapshot,
effects/nesting. This is strengthened **168 template modes**, not unrestricted
random AST generation or proof of all alias shapes. Fixed template portions and
small random values remain limitations; no shrinking/coverage-guided claim.

## Corpus and tokenizer commands

```sh
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/corpora/application-v5/tests
# Ran 22 tests; OK. Negative-oracle tests deliberately print FAIL examples.
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-algorithms/verify-all.json
# TOTALS tasks75 nil_programs56 unsupported19 reference_checks478 native_checks956 python_checks483 cpp_checks483
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-algorithms/verify-original.json
# TOTALS tasks25 nil_programs24 unsupported1 reference_checks120 native_checks240 python_checks125 cpp_checks125
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-algorithms/verify-all.json --output /tmp/nil-algorithms/density
# Adaptation examples: 56; QWEN_CROSSCHECK comparisons 170 mismatches []
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/attribution.py --density /tmp/nil-algorithms/density/density.json --output /tmp/nil-algorithms/attribution
# ACCOUNTED source_files 64 tokenizer_streams 256 tokens 44496 exact_sum_checks: passed
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/algorithms_results.py --density /tmp/nil-algorithms/density/density.json --attribution /tmp/nil-algorithms/attribution/summary.json --output /tmp/nil-algorithms/predicted-actual.json
# ATTRIBUTION_MOVEMENT_EXACT_SUM_CHECKS passed
```

All **2400** checks pass; original **610** separately. Native totals split evenly
between O0/O2. Sort-only external verification before each passed **1790** checks.
Verification source hashes show exactly eleven changed external `.nil` references,
with every original NIL, Python/C++, contract, oracle, golden and helper unchanged.
No support change: **19 unsupported (18 external)**. [Per-task disposition](../../benchmarks/corpora/application-v5/ALGORITHMS_ATTEMPTS.md).
56 verified references/exported positives remain a small adaptation seed.

Pinned Gemma/Qwen GGUF vocabularies, llama-cpp-python 0.3.16, tiktoken 0.12.0,
Python 3.12.11; official Qwen tokenizers 0.22.1 cross-check has zero mismatches.
Hashes and complete per-task bytes/characters/tokens are in the density JSON/CSV.
No inference, syntax tuning, task selection after counts or modified baseline.
The new ledger recognizes each's loop delimiters; baseline category totals stay
unchanged. Integer and exact rational movements both sum to −398 cl100k tokens.

## Performance, quotas and reproduction

Run measurements serially after other jobs finish:

```sh
./scripts/bench.sh runtime --full --workloads --workload newlines --workload append --repeats 9 --output /tmp/nil-algorithms/controls-workloads
./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-algorithms/controls-transform
benchmarks/paired/.venv/bin/python benchmarks/application/keyed/run.py --output /tmp/nil-algorithms/controls-keyed --repeats 9
benchmarks/paired/.venv/bin/python benchmarks/application/algorithms/run.py --output /tmp/nil-algorithms/scaling --repeats 9
benchmarks/paired/.venv/bin/python benchmarks/application/algorithms/boundaries.py --output /tmp/nil-algorithms/algorithm-boundaries
```

[Raw samples, provenance, source hashes, all controls/scaling and boundary output](../../benchmarks/reports/2026-10-03/V5_ALGORITHMS_PERFORMANCE.json).
16 MiB scan 7.6435 ms, 1 MiB append 26.1005 ms, 16 MiB transform 38.251583 ms.
Map insert1k/4k/16k/64k:3.952083/2.963458/4.60625/11.296792ms.
Sorting 1 Mi integers (construction+order verification included): 40.641 ms;
1 Mi integer each/reduction 3.347 ms. 64k maps sort+each 16.432 ms, each 13.616 ms:
owned key copies are still charged and visible. Sorting is O(n log n), each scans
linearly plus owned map-byte materialization/lookup. Re-sorting inside a loop still
repeats sorting; retained aliases still copy. No benchmark-specific reuse proof.

Against the last run, scan −1.4%, append +8.7%, transform +1.1% medians. We do **not**
claim zero numerical regression. The same four old workloads emit identical
executable LLVM IR against `a839574` except unused sort declarations; the C runtime
only adds sort helpers unused by those controls. Startup/OS/layout effects remain
unisolated. All samples retained, no rerun selected for a favourable number.
Transform byte-exactness/double roundtrip and append full content are verified.

Measured standalone sort bytes 33554392 / next E013; buffer 4194299 / next E013.
Decimal-key sorted map 262144 succeeds; 262145 E013 (full result reservation).
Insertion ceiling 262145 unchanged; 262146 E013. Other live values lower these limits.
These are exact checked operation-specific boundaries under the unchanged 64 MiB,
not a payload-size guarantee or borrowed-view semantics.

## Local history and stop boundary

Conventional commits separate preregistration, implementation, tests, corpus
rewrites and measurements. Both repositories commit locally; **nothing is pushed**.
Runtime reuse remains guarded by existing last-use and dynamic root uniqueness;
no unsupported alias proof or unchecked fast path was introduced. Map-owned byte
costs, repeated sorting and remaining Python token losses are explicitly retained.
No follow-up feature or model adaptation is started.
