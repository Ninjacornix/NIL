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
| `./scripts/ci.sh` | 466 passed, zero warnings | final-ci-debug-2.log |
| `./scripts/ci.sh release` | 466 passed, zero warnings | final-ci-release-2.log |
| `./scripts/sanitize.sh` | 199 passed, all suites clean | final-sanitize-2.log |
| `./scripts/fuzz-v5.sh --seed 102061801 --out /tmp/nil-round18/final-fuzz-102061801` | 475 programs/950 builds, all families, zero divergences | final-fuzz-102061801.log |
| same command, seed 74018329 | same counts, zero divergences | final-fuzz-74018329.log |
| same command, seed 918273645 | same counts, zero divergences | final-fuzz-918273645.log |
| `python3 benchmarks/corpora/application-v5/verify.py --output /tmp/nil-round18/final-corpus.json` | 2,815 checks | final-corpus.log/json |
| same command with `--group original` | exactly 610 | final-original.log/json |

Native regressions cover O0/O2, bounded/unbounded execution, aliases, effect order,
unselected effects, copying fallback and preheader/observed-helper IR. Host stdout
and files are compared by fuzzing. No frozen corpus file or std definition changed.
Earlier profiles/default keep their existing semantics and passing golden suites.
No syntax, intrinsic, dependency or quota change.

The no-regression performance gate remains unresolved for source sort, whose
medians are 4–9% higher in follow-ups. All timing campaigns and isolation controls
are retained in the full report; overlapping ranges alone are not treated as
parity. Round 18 is not declared complete.
