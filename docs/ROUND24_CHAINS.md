# Round 24 — Part B checkpoint

**Natural qualification remains 19/150; tuned qualification measured 23/150.**
The natural heap median improved from 64.268 to 54.093 ms, within the registered
35–60 ms forecast, but remains 5.01× C++. Other cross-region copy cliffs persist.
Std swap/pop remain 3.5–3.7× / 1.8× the equivalent caller forms; Part C is pending.

ADR 048 precedes the implementation commits. Contiguous update-chain operands
now evaluate left-to-right before writes; grouping preserves chains, explicit
bindings/reads/call/control results end them. Duplicate targets use the last
write. This deliberately changes fault/effect order in expr-v5. Earlier profiles
retain eager ordering; spellings/token costs and the 35 intrinsic variants stay
unchanged. No frozen natural/tuned source, task, oracle or baseline was edited.

The final implementation passes 577 tests in debug and release, zero warnings,
265 ASan/UBSan tests, corpus 2,815 / original 610 and both NeetCode cohorts 1,070.
Seeds 24311/24312 each exercise all 765 families (32 new chain families), with
zero non-resource divergences: 112 joint escalations, 76 resolved matches,
36 persistent E008, zero persistent E013/accounting failures. Host effects and
input preservation are compared on every outcome.

Two old families change deliberately under ADR 048: `g2_byte_trap_first`
E014→E012 and `g2_quota_before_read` E013→E012, because the later `a[-1]`
operand now precedes writes. Those are exact reference/native matches.
CI also found a record-field append proof regression; following the concat
result's sole use instead of requiring adjacency restores reuse across safe
scalar operands. Regression variants preserve all old-field/escape/alias guards;
intermediate records retaining children stay conservative, explicitly tested.

Isolated 25-repeat medians (Part A archive / Part B / C++): append 1 MiB
4.450 / 4.410 / 3.400 ms; historical scan **17 MiB** 7.451 / 7.466 / 9.990 ms;
transform 16 MiB 48.479 / 50.519 / 27.086 ms. All three emitted LLVM modules
are byte-identical; paired bootstrap delta intervals include zero. The absolute
transform elevation is disclosed. Output and double-transform are byte-exact.

The copy census is complete on its frozen Part B shadow compiler. Application:
68 programs / 157,626 copied bytes (+13); natural cohort: 400,057,040,260 bytes
(-293,337); tuned: 60,208,452 bytes (-293,337). The 399–400 GB natural forecast
narrowly missed, and no material aggregate improvement is claimed. Both fuzz
censuses execute all 765 programs with no skipped programs; each explicitly
lists 49 resource exclusions. Largest 3sum native traces exceed the existing
five-million-event join cap, as before, and are excluded rather than counted as zero.

[The full Part B report](../benchmarks/reports/2026-10-10/ROUND24_B.md) contains
min/median/max timing and RSS tables, before/after census, per-code/family resource
counts, forecast shortfalls and exact commands. Raw receipts are archived outside
git at `~/.local/share/nil/round24-chains/part-b/`; committed JSON records hashes.
Part C is in progress under the preregistered ADR 040 amendment. Nothing was
pushed. Round 24 remains open.
