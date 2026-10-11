# Round 24 — Part A checkpoint

**Natural ownership cliffs remain; std swap/pop are still slower than caller code.**
This step fixes stack containment and resource-test integrity, not those costs.
Parts B/C are pending; Round 24 is not complete.

The unbounded 200,000-depth recursion repro reports clean E008 at O0 and O2.
Large-record O0/O2 frames and the old ASan witness also pass. Non-guard bad
accesses remain crashes. Default native stack is 32 MiB; reference/bounded depth
is 16,384. Negative lengths are exact E014 errors, not resource failures.

Fresh gates: 563 debug/release tests, zero warnings; 259 ASan/UBSan tests;
36/36 nondivergent depth audit, zero signals; 2,815 corpus checks, original 610;
natural and tuned cohorts 1,070 each. Two seeds each cover all 733 families,
with zero non-resource divergences, 114 escalations, 78 resolved/matched pairs,
36 persistent E008 comparisons, no persistent E013 or rule-3 failures.

[The complete Part A report](../benchmarks/reports/2026-10-09/ROUND24_A.md)
contains per-code/family counts, exact commands, both control batches,
min/median/max cliff and std tables, RSS receipts and accepted limits.
Natural/tuned qualification remains 19/150 and 22/150. Nothing was pushed.
