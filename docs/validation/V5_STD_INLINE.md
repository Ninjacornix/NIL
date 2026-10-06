# Round 18 validation

Baseline `8647ba7`, branch `feat/std-inline`. No pushes. Independent audit commit
`c64df4b` precedes ADR039/040 commit `7ebaa8d`, which precedes `f80e4b1` (stable
updates) and `0d6fd7f` (linear source leaf expansion).

The [audit](ROUND18_AUDIT.md) records the observation-point invariant and its
single-threaded restrictions. The [full report](../../benchmarks/reports/2026-10-06/ROUND18.md)
records all measured controls, remaining losses, fuzz counts and SHA-256 receipts.
Raw output lives outside git under `/tmp/nil-round18/`.

| Command | Final result | Output receipt |
|---|---|---|
| `./scripts/ci.sh` | 466 passed, zero warnings | allocator-ci-debug.log |
| `./scripts/ci.sh release` | 466 passed, zero warnings | allocator-ci-release.log |
| `./scripts/sanitize.sh` | 199 passed, all suites clean | allocator-sanitize.log |
| `./scripts/fuzz-v5.sh --seed 102061801 --out /tmp/nil-round18/allocator-fuzz-102061801` | 475 programs/950 builds, all families, zero divergences | allocator-fuzz-102061801.log |
| same command, seed 74018329 | same counts, zero divergences | allocator-fuzz-74018329.log |
| same command, seed 918273645 | same counts, zero divergences | allocator-fuzz-918273645.log |
| `python3 benchmarks/corpora/application-v5/verify.py --output /tmp/nil-round18/allocator-corpus.json` | 2,815 checks | allocator-corpus.log/json |
| same command with `--group original` | exactly 610 | allocator-original.log/json |

Native regressions cover O0/O2, bounded/unbounded execution, aliases, effect order,
unselected effects, copying fallback and preheader/observed-helper IR. Host stdout
and files are compared by fuzzing. No frozen corpus file or std definition changed.
Earlier profiles/default keep their existing semantics and passing golden suites.
No syntax, intrinsic, dependency or quota change.

The initial 4–9% source-sort gap was recovered after the ADR039 allocation-site
addendum commit `54a447a` and runtime commit `398156d`. Final 25-repeat controls
are in `allocator-interleaved.json`: 1M source sort 1457.557→1437.751 ms; hand/std
map 35.737/36.187, transform 35.801. Push 78.733 versus concat 78.936 (0.997x).
Scan retains vector comparisons. Comparator insertion has ~1% positive drift;
the focused follow-up and baseline campaign variation are disclosed in the report.

All gates above were rerun after that runtime change. Additionally,
`NIL_CLANG=/tmp/nil-round18/fallback/clang ./scripts/sanitize.sh` passes 199 tests
with the portable allocation path forced by the private C test switch. No NIL
sanitizer finding occurred. Both standalone intentionally invalid C API probes
were correctly caught by ASan; they are not compiler/runtime bugs. The full
report records final hashes, all 90 map rows and the five unchanged E013 cases.
