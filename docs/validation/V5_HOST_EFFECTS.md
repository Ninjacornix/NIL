# Expr-v5 host effects: final validation

[ADR 033](../adr/033.md) was committed as `ae34bfc` before implementation
`1761538`. Three core host contracts were added: environment, injectable
randomness and deterministic immediate-directory snapshots. No new type, default
or earlier profile changed. Borrow-only providers reject them; modules use the
caller's Host and arena. The intrinsic enum is **32 -> 35**; its preregistration
count typo was corrected explicitly in history.

**One newly verified task, not the target three:** env_lookup. Entropy and grep
remain unsupported under their frozen property-oracle/fixture-adapter contracts.
Do not equate supplying host primitives with proving those application tasks.
[Full report and remaining-task classification](../../benchmarks/reports/2026-10-05/V5_HOST_EFFECTS.md).

## Reproduce

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
./scripts/fuzz-v5.sh --seed 5130572
./scripts/fuzz-v5.sh --seed 1729
./scripts/fuzz-v5.sh --seed 4294967295
```

Final output: **400 tests debug, 400 release**, no Clippy warnings. **159 tests**
passed across all eight native sanitizer suites, without a finding. Each final
fuzz run reports `programs:392`, `native_builds:784`, `divergences:0`: **1,176
programs, 2,352 builds** overall, 36 host families per seed, with complete
[per-code/per-family counts and log hashes](../../benchmarks/reports/2026-10-05/V5_HOST_EFFECTS_VALIDATION.json).

Random comparison is exact u64, never approximate: published SplitMix64 vectors,
selected/lazy calls, called functions and loop state agree at O0/O2. Native
`NIL_RANDOM_SEED=0` prints `16294208416658607535`, matching the reference vector.
Malformed seed, denial and OS entropy I/O failures are tested. The last is forced
on macOS using a test-only sandbox denying `/dev/urandom`; this is not a NIL
sandbox feature. Default reference execution still returns E018 for every new
effect without Host. Injected Hosts and native fixtures test raw env bytes,
ordered/empty/large directories, missing/unreadable paths and live quota admission.

The preliminary quota generator incorrectly consumed the old length before
allocation, allowing storage to die. Final generation reverses that order and
asserts E013 for both new quota families on all seeds. No compiler/runtime bug or
sanitizer finding was observed. A 0xff filename could not be created on this
macOS filesystem; raw-name injected-host tests and native raw environment tests
are distinct coverage. The checker initially inherited unrelated environment
variables after env was enabled; preflight and fixture execution are now scrubbed,
with a sentinel audit proving all five reference calls use controlled environments.

## Corpus and density

```sh
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/host-corpus.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/host-corpus.json --output /tmp/host-density
```

[Corpus verification](../../benchmarks/reports/2026-10-05/V5_HOST_EFFECTS_CORPUS.json)
passed **2,790 checks**: 558 reference, 1,116 O0/O2 native, 558 Python, 558 C++.
There are 79 unchanged tasks, 67 verified NIL programs/JSONL positives, and 12
unsupported tasks. The [original 610 subset](../../benchmarks/reports/2026-10-05/V5_HOST_EFFECTS_ORIGINAL_610.json)
retains its pre-existing environment baseline checks; only 5 reference/10 native
checks were added. All 210 pre-existing contract, baseline and solution files
were byte-identical to the starting benchmark revision. No oracle/golden changed.

Density is recorded across Gemma, Qwen, cl100k and o200k with versions/digests,
bytes/characters and [per-task token tables](../../benchmarks/reports/2026-10-05/V5_HOST_EFFECTS_TOKENS.csv).
Original 25 now use **6.2–10.6% fewer Python tokens**; external 42 still use
**21.9–31.3% more**, combined 67 **15.0–22.9% more**. The old 66 source/counts
are unchanged. This cohort addition does not demonstrate syntax improvement or
model token efficiency; the failed generation/few-shot study remains unchanged.

## Performance and local history

Same warm whole-process methodology, 25 repeats, rotating languages, compilation
excluded, output verified, no fsync/CPU isolation:

```sh
./scripts/bench.sh runtime --full --workloads --workload append --workload newlines --repeats 25 --output /tmp/host-controls
./scripts/bench.sh runtime --full --application --repeats 25 --output /tmp/host-transform
```

| NIL control | Previous ms | Current ms | Current min–max ms |
|---|---:|---:|---:|
| Append 1 MiB | 22.899 | 22.839 | 22.230–30.831 |
| Scan 16 MiB | 7.728 | 7.560 | 7.416–8.057 |
| Transform 16 MiB | 36.819 | 36.186 | 35.257–39.986 |

No observed regression; variability does not prove a speedup. C++/Python controls,
all samples and stage profiles are linked in the full report. All three transform
sizes round-trip exactly. Existing aliased/nested builder and provider-matcher
limits remain; no optimization, quota increase or weakening of checks was made.

All work is committed locally on `feat/application-core` and the benchmark
submodule's `exp/application-storage`. No push was performed; origin refs remain
`ac92743c…` and `57bf1354…` respectively. Reports contain reproducible commands;
raw logs are archived outside the repository with hashes in the receipts.

Raw log/archive: `/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-host-effects-2026-10-05.tar.gz`. SHA-256: `3880625b2d27565271d6be7fe767cbd6d2cc7a4fea8ea2c9f98d00e4e70ba88d`.
