# Round 16 — guarded ownership validation

**Filter, transform and comparator insertion sort still miss the C++ targets.**
This is a partial improvement with the original immutable-value guarantees.

ADR 036 was accepted in `73d694a` before backend changes `24dbb69` and `94a5f9b`.
The 52 update-return call sites include 41 semantic last uses and only seven
closed fresh call-entry uniqueness proofs. Runtime exactly-one-root checks
remain mandatory; last use alone is not a physical alias proof.

Exact historical 25-repeat medians (ms):

| Control | Before | After | C++ |
|---|---:|---:|---:|
| Append 1 MiB | 22.090 | 4.475 | 3.530 |
| Readonly scan 16 MiB | 7.470 | 7.497 | 10.216 |
| File transform 16 MiB | 36.499 | 36.568 | 13.739 |

Std filter improves 413.389 to 274.521 ms against C++ 30.104; comparator insertion
sort improves 1019.821 to 60.755 against same-algorithm C++ 4.748. Source comparator
merge sort at 1M remains 1360.087 versus core 131.151 and C++ 96.414 ms; core sort
stays. Readonly SIMD survives; checked writable loops do not vectorize. Allocating
lazy regions and frequent callee root handoffs remain conservative.

Executed gates, with receipts under `/tmp/nil-ownership/`:

- `./scripts/ci.sh`: **455 passed**, zero Clippy warnings (`gates-ci-debug.log`).
- `./scripts/ci.sh release`: **455 passed**, zero warnings (`gates-ci-release.log`).
- `./scripts/sanitize.sh`: **188 passed**, no ASan/UBSan findings (`gates-sanitize.log`).
- `./scripts/fuzz-v5.sh --seed SEED --out /tmp/nil-ownership/gates-fuzz-SEED`:
  seeds 5130572, 907221, 160016, each 461 programs/922 O0/O2 builds, all family
  indices exercised, zero divergences; host effects and diagnostic codes compared.
- Benchmark environment `python benchmarks/corpora/application-v5/verify.py
  --output /tmp/nil-ownership/post-change-corpus.json`: **2,815 checks**.
- Same command with `--group original` and a separate output: **exactly 610**.
- `python -m unittest discover -s benchmarks/generation/standing/tests`:
  **46 passed**, mock reports confined to temporary directories.

14 behavior tests in `crates/nil-llvm/tests/ownership.rs` cover aliases, bounds,
byte/quota failures, effects, unselected lazy arms, recursion/depth, left-to-right
arguments, empty/zero-slot layouts, stable source sort and retained-read ranges.
23 new fuzz families cover the corresponding transfer/builder boundaries.
No runtime divergence or sanitizer bug was found. One malformed generator case
was corrected; an old copying-path IR test now uses two old reads because the
single-read shape is provably snapshot-compatible. No frozen oracle was changed.

[Full min/median/max tables, forecasts, corpus timings, IR evidence,
per-code/family counts, hashes and remaining limits](../../benchmarks/reports/2026-10-06/OWNERSHIP.md).
Compiler parser/HIR, corpus sources, tasks, goldens and baselines are unchanged.
Intrinsic count remains 35 variants / 34 executable core bodies. All commits are
local on `feat/ownership` and benchmark `exp/ownership`; nothing was pushed.
