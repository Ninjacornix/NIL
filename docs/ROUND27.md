# Round 27 — faster gates, same evidence

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
| corpus | ~260 s | ~260 s | P3 not done |
| corpus original | ~90 s | ~190 s | P3 not done |
| cohorts (two) | ~315 s | ~315 s | P4 not done |
| depth | ~20 s | ~20 s | untouched |
| **full, sequential sum** | **~73 min** | — | |
| **full, one run of `gates.sh full`** | — | **22.8 min** | 1,368.5 s, 7-job budget |
| **fast, one run** | — | **11.9 min** | 713.1 s |

So the full suite goes from roughly 73 minutes of sequential gates to **22.8
minutes measured end to end**, about **3.2×**, of which the two real speedups
are fuzz and the sanitizer and the rest is concurrency.

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
```

Submodule `exp/gates`:

```
dcd4e70 feat(gates): add NIL_PREBUILT so Python gates never rebuild
```

Branches `feat/gates` and submodule `exp/gates`. Nothing pushed.
