# Round 24 — guarded stacks, simultaneous chains, checked std expansion

**Natural qualification remains 19/150; tuned measures 23/150.** The natural
copy cliffs remain: simultaneous chains remove only 293 KB from about 400.057 GB
of native O0 copied payload. No frozen source or held-out problem was edited.
This round does not establish general C++ parity.

## Stack and resource outcomes

RT-1 is fixed: `=#b(200000)` with
`1:s=a<1?!bytes(1,0):!concat(b(a-1),!bytes(1,1))` reports clean E008 in
unbounded native O0/O2. Programs run on a configurable 32 MiB guarded stack;
only a fault in its guard page becomes E008. A genuine non-stack bad access
still crashes. The reference evaluator uses heap frames; its default depth
guard is 16,384. Native exhaustion depth is an ordinary environment limit.

Negative lengths now report deterministic E014 before fill checks. E008/E013
comparisons escalate both paths with all limits jointly 4×, after checking
host effects and input preservation. The old 88 E013 comparisons are accounted
for: ten are now exact E014, two later-operand failures are exact E012 under
ADR 048, and the remaining 76 resolve at 4×. None are silently dropped.

## Chains and std expansion

ADR 048 evaluates the receiver, then every contiguous-chain index/value
left-to-right before writes. Writes commit in order, with the last duplicate
winning. Grouping is transparent; explicit bindings, projections, calls and
control results form boundaries. Unselected lazy arms remain unevaluated.
This intentionally changes expr-v5 fault/effect ordering; earlier profiles keep
their previous meaning. Spellings and token costs are unchanged.

ADR 040's preregistered amendment expands small flat checked sequence leaves
before rooting. SSA arguments are evaluated once; repeated reads do not duplicate
argument expressions. Cloned instructions preserve order and spans, HIR revalidates,
and existing alias/liveness guards decide reuse. Bounded calls remain instrumented.
No std-name recognizer or new intrinsic was introduced: the count stays 35.

25-repeat O2 timings, min/median/max and raw samples in the linked full report:

| Operation | Part B std median ms | Final std ms | Caller ms | Ratio |
|---|---:|---:|---:|---:|
| Bytes swap | 23.710 | 6.308 | 6.316 | 0.999× |
| Buffer swap | 23.945 | 6.557 | 6.554 | 1.001× |
| Bytes pop | 12.901 | 6.906 | 7.245 | 0.953× |
| Buffer pop | 13.369 | 7.496 | 7.476 | 1.003× |

All four meet the within-10% forecast. Call-site costs remain swap 6 / pop 4
tokens on cl100k/o200k. Entry IR has one source-callee call before and none after.
An exact-E013 test assumption was converted to a larger-limit reference comparison;
a separate effectful live-alias witness still requires a real copy. No unexplained
non-resource divergence was found. CI also found and fixed a record-field append
reuse regression caused by chain lowering, with variant and alias regressions.

## Natural cliffs and controls

15-repeat O2 medians; C++ uses the frozen reference algorithm:

| Natural program | NIL ms | C++ ms | Ratio |
|---|---:|---:|---:|
| Balanced binary tree | 1481.530 | 13.332 | 111.13× |
| Course schedule | 4315.198 | 22.244 | 193.99× |
| Last stone weight | 53.104 | 10.674 | 4.98× |
| Spiral matrix | 27.424 | 7.272 | 3.77× |
| Validate BST | 1411.189 | 12.697 | 111.14× |

25-repeat control medians: append 1 MiB 4.449 ms versus Part B archive 4.450;
historical scan **17 MiB** 7.489 versus 7.427; transform 16 MiB 39.003 versus
41.922. All three LLVM modules are byte-identical. The separately measured true
16 MiB scan is 7.016 ms. Transform output is byte-exact and double-transform
round-trips. Timing/RSS ran alone, after correctness; trace instrumentation was excluded.

## Validation and accepted limits

Final receipts: debug/release 579 tests, zero warnings; ASan/UBSan 267 tests;
depth audit 36/36 with no signals; permanent I/O audit 14 families / 168 comparisons
(72 exact, 96 resource-inconclusive, zero diagnostic divergences). Corpus 2,815,
original exactly 610; both NeetCode cohorts 1,070 each, zero corpus inconclusives.

Seeds 24411/24412 each exercise all 773 families and 1,658 native builds, with zero
non-resource divergences or accounting failures. Each escalates 112 comparisons:
76 match, 36 remain E008 (large-record both 12, sequence both 12, mutual both 10
plus reference-only 2). No persistent native-only E013 is accepted.

General interprocedural G1/path correlation remains deferred. The BST scalar
keeping read crosses a sequence-valued lazy region; union liveness retains the
original. Record-buffer swap/pop remains E007 pending child-edge proofs.
Large, recursive, aggregate and effectful leaf bodies stay conservative; bounded
builds keep calls. POSIX stack implementation is locally validated on arm64 macOS,
not independently on Linux/x86. Remaining collection-builder and update-vectorization
gaps are not silently claimed solved.

[The full report](../benchmarks/reports/2026-10-10/ROUND24.md) contains complete
before/after census, per-family/code resource counts, min/median/max and RSS,
forecast comparisons, commands and artifact hashes. Raw receipts are archived
outside git at `~/.local/share/nil/round24-chains/final/`. Each fuzz census ran
all 773 programs with zero skips and 49 named resource-excluded fixtures; these
are distinct from the 36 differential E008 comparisons. ADRs precede their
implementation commits. All commits are local; nothing was pushed.
