# Round 23 — fail-fast acquisition amendment validation

The 2026-10-08 maintainer decision is committed before code in `5e3013d`
(ADR 045) and `6a7d54e` (ADR 033). An oversized successful prefix now produces
reference E013 before a later E015, at the existing bang span. Native behavior
is unchanged. This intentionally resolves the historical negative admission
report; it does not implement the logical ledger or admit B2–B4.

## Implementation and permanent gates

`Host::read_with_limit` supplies the admissible payload; FileHost streams at most
that amount plus one witness byte through `read_prefix`. Earlier errors remain
E015. SeededHost forwards the method; denial remains E018. Existing materialized
Hosts retain a compatibility default; streaming Hosts must override it because
an opaque failed whole-read result contains no prefix information.

The existing audit retains all fourteen families and uses the shared test-only
interposer in `tools/nil-fuzz/fixtures/read_fault.c`. `scripts/read-audit.sh`
requires parity and nightly runs a mandatory macOS job. It tests actual FileHost,
not a substitute reference Host. The generator adds fourteen related families
with real native faults and a prefix-failing in-memory reference Host; it compares
prefix bytes and EIO delivery in addition to codes/spans, stdout and files.
The audit is Darwin-specific. The shared fixture includes a Linux/glibc path,
not locally verified; other platforms fail closed. Nightly now requests full
family coverage instead of an obsolete hard-coded count.

## Command receipts

Raw artifacts are outside git at `~/.local/share/nil/round23-memory/precedence/`.
All commands below returned zero. The audit JSONs include build commands, source
and executable hashes, code/span observations and fault receipts per run.

| Command (output path abbreviated) | Output |
|---|---|
| `./scripts/ci.sh > …/ci-debug.log` | 532 tests; strict Clippy; zero warnings |
| `./scripts/ci.sh release > …/ci-release.log` | 532 tests; zero warnings |
| `./scripts/sanitize.sh > …/sanitize.log` | 237 tests across all fourteen suites; no ASan/UBSan findings |
| `./scripts/read-audit.sh --out …/audit --repeats 3` | 14 families; 42 reference runs; 168 native comparisons; 0 divergences |
| Same audit with sanitizer `NIL_CLANG` wrapper, `--repeats 1 --out …/sanitized-audit` | 14 reference runs; 56 native comparisons; 0 divergences; no sanitizer findings |
| `./scripts/fuzz-v5.sh --seed 23011 --out …/fuzz-23011` | 687 programs; 1,374 native builds; 0 divergences; every family once |
| Same full campaign, seed 23012 | 687 programs; 1,374 native builds; 0 divergences; every family once |
| `python3 benchmarks/corpora/application-v5/verify.py --output …/corpus.json` | 2,815 checks: reference 563, native 1,126, Python 563, C++ 563 |
| Same verifier, `--group original --output …/original.json` | Exactly 610: reference 120, native 240, Python 125, C++ 125 |
| `python3 benchmarks/corpora/neetcode150/verify.py --verified-only --jobs 1 --output …/tuned.json` | 29 programs; 1,070/1,070 checks |
| Same NeetCode verifier, `--candidate-dir "$PWD/benchmarks/core-generalize-study/natural" --output …/natural.json` | 29 programs; 1,070/1,070 checks, run alone |

The sanitized audit wrapper executes `/usr/bin/clang` with
`-fsanitize=address,undefined,float-cast-overflow -fno-sanitize-recover=all
-fno-omit-frame-pointer`; ASan disables unsupported macOS leak detection and both
sanitizers halt on errors. Its exact script and logs are archived beside receipts.

## Differential coverage and limits

Both full seeds report these executed codes (per seed):

| Code | Count | Code | Count |
|---|---:|---|---:|
| OK | 502 | E009 | 8 |
| E012 | 27 | E013 | 46 |
| E014 | 10 | E015 | 12 |
| E016 | 14 | E017 | 10 |
| E018 | 11 | E019 | 3 |
| E020 | 6 | E021 | 35 |
| E022 | 3 | E008 | 0 |

The new fourteen families yield E013 eight times, E015 four, OK twice per seed:
quota-real-read, quota-before-late-fault, buffer retention, record child,
record-buffer child, map+child, selected lazy arm and callee retention hit E013;
no pressure, below quota, before any prefix and dead-value cases hit E015;
short-file success and unselected arm succeed. Each family is individually
reported in `family_codes`/`call_families`. There are 505 reported semantic and
manifest-family tags, separate from the 687 source-family indices.

Each seed has 679 distinct source hashes; union 771, intersection 587, 92 unique
to each seed. Seeds principally vary values, not the whole structural matrix.
These are controlled faults, not observed disk failures or a proof over all I/O.
The full generator uses unbounded native execution, so its zero E008 count is not
a depth-boundary claim; bounded/depth regressions still pass CI and the audit.

No native compiler/runtime/std source changed for the amendment. Earlier/default
profiles and frozen corpus sources, contracts, oracles, goldens and baselines are
unchanged. Five new reference tests and one generator-matrix test account for the
526→532 test increase. No additional compiler defect or sanitizer finding was
found during this fix. Natural 19/150 and tuned 22/150 are carried-forward
qualification counts: this amendment claims no performance improvement.

## Next step

Resume B1 under ADR 045. The maintainer additionally requires an all-bytes physical
bound: allocate a 40-byte ledger descriptor only when replacing an existing
40-byte-header-plus-payload copy, no descriptor on existing no-copy paths and no
per-value extra storage on unshared values. Report descriptor counts/bytes, RSS
and logical/physical charge separately. B1 is not accepted merely because this
precedence audit now passes; its independent lifetime and quota proof remains.
