# Token-attribution validation — 2026-10-03

No compiler/runtime/profile/grammar or corpus source changed. [ADR 024](../adr/024.md) records the analysis-only decision. The attribution and forecasts were committed first: benchmark `6060f9e`, main `908d02e`. Later commits publish reproducibility/evidence only; no implementation commit follows. Nothing was pushed.

## Attribution and density receipts

```text
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/attribution.py --density benchmarks/reports/2026-10-03/V5_KEYED_DENSITY.json --output /tmp/nil-attribution/analysis
ACCOUNTED {"source_files":64,"tokenizer_streams":256,"tokens":46236,"exact_sum_checks":"passed"}
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/remediation.py --output /tmp/nil-attribution/analysis
signature_references: Gemma 78 Qwen 32 cl100k 32 o200k 32
empty_loop_steps: Gemma 131 Qwen 9 cl100k 9 o200k 9
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/corpora/application-v5/tests
Ran 20 tests; OK
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-attribution/verification.json --output /tmp/nil-attribution/density
Adaptation examples: 56; QWEN_CROSSCHECK comparisons 170 mismatches []
FROZEN_DENSITY_ALL_THREE_CUTS_IDENTICAL; ALL_SOURCES_AND_ORACLES_IDENTICAL
```
Full [category accounting and pre-implementation ranking](../../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_PLAN.md), [forecast sources](../../benchmarks/reports/2026-10-03/V5_SURFACE_FORECASTS.json) and [all three density cuts](../../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_RESULTS.md) are committed. Numeric literal surplus is 394 cl100k tokens; most are already single-token constants. Signature and empty-step proposals are unimplemented, invalid syntax in the current compiler and excluded from adaptation export. Actual implemented savings: 0; executable forecast-validation is not applicable. No existing verified program’s meaning changed.

The primary ledger uses exact rational byte-overlap shares for mixed tokens; the secondary whole-token ledger has integer sums. Both exhaustively conserve totals. Lexical categories do not prove a universal minimum or a counterfactual semantic saving. Python library calls are classified alongside NIL intrinsics. Repeated-expression bytes are not double-counted as an additive category; the overseer’s 243 combined-token ceiling is not a forecast of safe binding savings.

## CI, native parity, sanitizers

| Command | Exit | Cited output |
|---|---:|---|
| `./scripts/ci.sh` | 0 | 239 passed; no warnings |
| `./scripts/ci.sh release` | 0 | 239 passed; no warnings |
| `./scripts/sanitize.sh` | 0 | 55 passed; no warnings |

Debug CI includes rustfmt and Clippy with `-D warnings`. Both native sanitizer suites ran: application 36, keyed 19; tests include reference/native O0/O2 parity, quota/trap priority, aliases and effects. ASan/UBSan found no invalid access/UB. LeakSanitizer is disabled on macOS; this is not a leak audit. Compiler test count remains 239 because no compiler code or Rust test was added. Nine benchmark analysis regressions bring the separate Python corpus suite to 20.

```text
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-attribution/verification.json
TOTALS tasks75 nil_programs56 unsupported19 reference_checks478 native_checks956 python_checks483 cpp_checks483
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-attribution/verification-original.json
TOTALS tasks25 nil_programs24 unsupported1 reference_checks120 native_checks240 python_checks125 cpp_checks125
```
**2400 checks pass; original 610 separately.** Native counts split evenly between O0/O2. [Frozen verification](../../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_VERIFICATION.json) source hashes match the previous report exactly. Baselines, NIL sources, tasks, oracles and goldens are unchanged; existing original/keyed hash-lock tests pass. `git diff b83236b -- crates cli tools scripts Cargo.toml Cargo.lock` is empty, confirming the compiler was not touched.

## Three-seed differential fuzzing

```sh
for seed in 5130572 1729 8675309; do
  ./scripts/fuzz-v5.sh --seed "$seed" --cases 268 --out "/tmp/nil-attribution/fuzz-$seed"
done
```
| Seed | Programs | Native O0/O2 builds | Divergences |
|---|---:|---:|---:|
| 5130572 | 268 | 536 | 0 |
| 1729 | 268 | 536 | 0 |
| 8675309 | 268 | 536 | 0 |

Each seed reports the same enforced outcome coverage; generated values/keys vary with seed:

| Code | Each seed | All three |
|---|---:|---:|
| E012 | 22 | 66 |
| E013 | 20 | 60 |
| E014 | 10 | 30 |
| E015 | 6 | 18 |
| E016 | 18 | 54 |
| E017 | 6 | 18 |
| E018 | 6 | 18 |
| E019 | 2 | 6 |
| E020 | 6 | 18 |
| OK | 172 | 516 |

[Complete per-seed call-family and intrinsic counts](../../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_FUZZ.json) preserve every reported family. All call_families have two cases per seed, including called lazy effects, sequence-returning/recursive calls, computed indices, map aliases, insertion order, missing/duplicate/quota priority and returned owned bytes. No source-mutation extension was required because syntax did not change. This is 134 known template families with two samples each, not unrestricted random AST generation. No compiler divergence/bug was found.

## Performance controls

Run serially after correctness/fuzz jobs; one warmup, medians of nine whole-process runs, cached file I/O, no fsync/CPU isolation. Baseline is the previous keyed-data report using identical workloads. Every output is verified; transform double-roundtrip is true at all sizes. Source hashes are retained; dirty provenance flags reflect only analysis/docs/reports, not compiler/workload changes. No optimization was performed.
```sh
./scripts/bench.sh runtime --full --workloads --workload newlines --workload append --repeats 9 --output /tmp/nil-attribution/controls-workloads
./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-attribution/controls-transform
benchmarks/paired/.venv/bin/python benchmarks/application/keyed/run.py --output /tmp/nil-attribution/scaling --repeats 9
```
| Workload | Previous NIL ms | Fresh NIL ms | Fresh C++ ms | Fresh Python ms |
|---|---:|---:|---:|---:|
| newlines, 16777216 bytes/iterations | 7.476 | 7.755 | 10.261 | 23.969 |
| append, 1048576 bytes/iterations | 24.088 | 24.018 | 4.802 | 43.209 |
| transform, 16777216 bytes/iterations | 38.117 | 37.832 | 14.411 | 781.499 |

NIL still loses to C++ on append and transform, wins this scan. The fresh scan median is 3.7% higher than the previous one, append 0.3% lower, transform 0.7% lower; these uncontrolled timing differences do not establish a code regression or improvement. The compiler/workloads are byte-identical. All raw nine-sample measurements and additional sizes/stages are in the [performance JSON](../../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_PERFORMANCE.json).

| Map insertions | Previous NIL ms | Fresh NIL ms |
|---|---:|---:|
| 1000 | 2.497 | 3.247 |
| 4000 | 2.924 | 2.915 |
| 16000 | 4.195 | 4.098 |
| 64000 | 10.219 | 10.232 |

The noisy 1k point is retained even though it exceeds 4k. Whole-process startup dominates small inputs; there is no CPU isolation or statistical regression threshold. The run also preserves both repack curves and all samples; different-length byte-value updates still copy the owned payload. Native decimal-key boundary again returns 262145, then `E013 @25..26 dynamic allocation limit exceeded` at 262146. No quota or ceiling change occurred.

## Raw output archive and local history

Raw command logs, full token ledger, forecast sources, density, verification and performance receipts: `/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-attribution-2026-10-03.zip`. SHA-256 `d45700d9928411be692296830f5e385eb9ec6446f1a85efed86fff1cd56c4439`. Regenerate with the commands above; no generated executables or fixture files enter Git.

Analysis and preregistered forecasts precede result commits. All commits remain local in `feat/application-core` and the existing benchmark submodule branch. No remote-ref update or push was performed.
