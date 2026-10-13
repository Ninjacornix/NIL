# Round 27 — faster gates, same evidence

## The reference evaluator was quadratic in call depth, and now is not

This blocked qualification, so it came first. **Diagnosed before fixing:**
`live_bytes` swept every value of every live frame on every interpreter step,
to sum live dynamic bytes, build per-pointer root counts and clear newly dead
values. That is O(depth) per call, so a depth-d recursion cost O(d²). A pure
recursion with no data structures at all measured:

| depth | before | µs/call | after | µs/call |
|---:|---:|---:|---:|---:|
| 1,000 | 0.42 s | 417 | — | — |
| 3,000 | 6.22 s | 2,073 | — | — |
| 10,000 | 45.65 s | 4,565 | 0.019 s | 1.89 |
| 30,000 | 299.94 s | 9,998 | 0.041 s | 1.36 |
| 100,000 | — | — | 0.124 s | 1.24 |
| 300,000 | — | — | 0.425 s | 1.42 |

Microseconds per call rose linearly with depth before and is flat after.
Restricting the sweep to the top frame flattened it, which confirmed the frame
iteration was the sole quadratic term before any fix was written.

**The fix.** Only the top frame can change between steps: a lower frame's
`next` does not advance and its values are not written. So the frames below the
top contribute a constant, computed once when each stops being the top and
subtracted once when it becomes the top again. Frames are LIFO, so a frame's
first-sight attribution is relative only to frames below it, which never change
while it is parked, and therefore subtracts exactly. Seeding the top sweep's
`seen` set from the parked pointers reproduces the original single `seen` set,
which is what keeps shared bytes counted once and a shared record buffer's
children counted once.

**Semantics unchanged, by the strongest check available.** Seed 61001 produces a
**byte-identical** fuzz summary,
`d1bf0c92f4fc57d7ce9f3068d1ab4b5ba84d775c8932ebb8fa064e023a4a0a6b`, so 843
programs still agree with native at O0 and O2 on results, diagnostics, host
effects and resource outcomes. E008 still fires exactly at the configured limit.

**Two programs newly qualify.** The depth-100,000 tree fixtures run the
reference path in **0.92 s** and **1.44 s** where they previously exceeded 400 s,
and both now report `verified 35/35`:

| Program | v2 kernel tokens | Python | vs Python | v2 / C++ (15 interleaved pairs) |
|---|---:|---:|---:|---:|
| balanced-binary-tree | 107 | 164 | **−34.8%** | **2.393×** |
| validate-binary-search-tree | 133 | 95 | +40.0% | **2.575×** |

Both are inside the 3× bound. `course-schedule` v2 still times out on the
reference path — **>400 s against v1's 26.35 s** — for the separate, already
recorded reason that nested containers copy on append; that is not a depth
problem.

A regression test guards the curve rather than a time, with a wide margin:
quadratic would show as 16× for a 4× depth increase, linear as 4×, and it fails
past 8×. Proven sensitive — against the old evaluator it fails at **9.13×**
(45.4 s and 414.4 s).

## Unfavourable first

**The fast gate misses its ten-minute target at 11.9 minutes.** The cause is
not slowness but correctness: the cohort harness enforces a **120 s wall-clock
timeout per check**, so run beside a saturating gate it reports spurious
reference timeouts. A concurrent fast run measured **1,067/1,070** and
**1,069/1,070**; serialized, both are 1,070/1,070. Cohorts are therefore
treated as serial, which adds 315 s after the concurrent block. Leaving them
concurrent would have hit 8.5 minutes and lied.

**Four of the nine planned items are not done:** P3 (corpus `--jobs`,
`--original-output`), P4 (cohort `--cohort name=DIR`, per-source-hash caching,
`--timing-from-receipt`), P6 (`NIL_BUILD_CACHE`) and P8 (fuzz family registry).
P3 and P4 are where the remaining minutes are — corpus is 260 s and the two
cohorts 315 s, all still single-threaded.

**The old full path was never a script**, so there is no end-to-end baseline
receipt to compare against. The before column below is a **sum of measured
component times**, not one timed run, and is labelled as such.

## Before and after

| Gate | before | after | note |
|---|---:|---:|---|
| `ci.sh` | ~400 s | ~400 s | untouched |
| `ci.sh release` | ~400 s | ~400 s | untouched |
| `sanitize.sh` | **1,082.0 s** | **407.1 s** | P2, 2.66× |
| fuzz seed 61001 | **907.4 s** | **310.5 s** | P1, 2.92× |
| fuzz seed 83003 | ~907 s | ~310 s | P1 |
| corpus + original | ~240 s (two runs) | **32.2 s** | P3, both groups in one run |
| cohorts (two) | ~315 s serial | ~315 s, now **concurrent** | CPU budget, P4 caching not done |
| depth | ~20 s | ~20 s | untouched |
| **full, sequential sum** | **~73 min** | — | |
| **full, one run of `gates.sh full`** | — | **22.6 min** | 1,355.7 s, all gates green |
| **fast, one run** | — | **11.9 min** | 713.1 s, cohorts were serial then |

So the full suite goes from roughly 73 minutes of sequential gates to **22.6
minutes measured end to end**, about **3.2×**. The real speedups are fuzz, the
sanitizer and the corpus; the rest is concurrency, and the effective-core budget
matters more than the gate count — cores+2 measured 1,355.7 s against 1,681.8 s
at cores−2, both fully green.

Final full receipt at `/tmp/gates-w10/receipt.json`: ci **638**, release
**638**, sanitizer **305** with 0 findings, both seeds **843 programs, 0
non-resource divergences, 0 rule-3 failures**, corpus **2,815**, original
**exactly 610**, both cohorts **1,070/1,070**, depth **36/36**.

## Equality proofs

Every speedup had to produce the same evidence before the old path was retired.

* **P1 fuzz `--jobs`.** Seed 61001 at `--jobs 5` produces a **byte-identical**
  summary to the pre-change build, sha256
  `d1bf0c92f4fc57d7ce9f3068d1ab4b5ba84d775c8932ebb8fa064e023a4a0a6b`. The
  extraction of `case_delta` was separately proven byte-identical *before*
  `--jobs` existed, and `--jobs 1` matches `--jobs 4` on the chains scope. The
  licence for this is that the case body only writes its accumulators — one
  self-read of `escalated_programs` aside — so cases are a pure fold; per-case
  directories are keyed by `seed + index` and the native build path numbers its
  temp directories with an `AtomicU64`.
* **P2 sanitizer.** All **18** previously covered targets report unchanged pass
  counts, proved by comparing the per-target count multisets: nothing is
  missing. The total rises **285 → 305** because the old hand list silently
  omitted the `native` target, which contributes exactly 20 tests. More
  coverage, not different coverage.
* **P5 `NIL_PREBUILT=1`.** A missing artifact is a hard error, not a quiet
  rebuild, and the SHA-256 of each measured binary goes in the receipt.
* **P7 full run.** ci 635, release 635, sanitizer 305 with 0 findings, both
  seeds 843 programs with 0 non-resource divergences and 0 rule-3 failures,
  corpus 2,815, original **exactly 610**, both cohorts 1,070/1,070, depth
  36/36 — every count equal to the hand-run suite.
* **P7 `--expect`.** Verified to fail with exit 1 and name the gate:
  `REGRESSED ci.tests: 635 < 9999`.

## P9, and why it exists

`scripts/ab_time.py` alternates OLD and NEW one pair at a time and reports the
ratio of medians over at least fifteen pairs, refusing fewer and refusing two
binaries that disagree on output. It exists because sequential measurement on
this machine is not trustworthy: the *same* binary measured 5.080× and 5.735×
against C++ in two sessions with byte-identical code, and a first version of the
Round 26 collection gate was reported as a 20% transform regression that
interleaving showed to be noise. Checked against identical binaries it reports
**1.004×**, and it independently reproduces the Round 26 collector result at
**2.47×** (16,692 → 6,748 ms).

Copy census is no longer a routine gate.

## Commits

```
209c896 perf(fuzz): run campaign cases in parallel behind --jobs N
c9ddca3 perf(gates): discover sanitizer targets and run them four-way
8896c2c feat(gates): add scripts/ab_time.py for interleaved A/B timing
ba9b648 feat(gates): add scripts/gates.sh fast|full with a receipt
0aa4503 perf(evaluator): make reference call cost O(1) in depth, not O(depth)
0e0b211 feat(gates): add --seeds so a reviewer can use independent fuzz seeds
077ac07 perf(gates): let cohorts run concurrently now their budget is CPU time
869ab31 fix(gates): schedule gates by effective cores, not gate count
33faea4 docs: record the evaluator fix, the two newly qualifying programs and the scheduler finding
```

Submodule `exp/gates`:

```
dcd4e70 feat(gates): add NIL_PREBUILT so Python gates never rebuild
4a8e0de perf(gates): run corpus tasks concurrently and derive the original group
1a472e1 fix(core): make the per-check cohort budget CPU seconds, not wall seconds
```

Branches `feat/gates` and submodule `exp/gates`. Nothing pushed.
