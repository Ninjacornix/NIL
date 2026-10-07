# Round 21 — measurements before generalization

Status: **measurement and preregistration complete; no compiler change shipped.**
Main branch `feat/core-generalize` started at `2e6614e`; benchmarks branch
`exp/core-generalize` started at `291d3c9`. Neither has an upstream. Nothing is
pushed. No compiler, runtime, CLI, std, frozen corpus, oracle or golden changed:
`git diff 2e6614e..HEAD -- crates cli tools scripts examples plugins` is empty.

See [ROUND21.md](../benchmarks/reports/2026-10-07/ROUND21.md) for the full report
and every gate result.

## Natural qualification is the headline

Benchmarks commit `e9a39c6` freezes 29 byte-exact Round 19 admitted sources from
`6fd268d`, alongside the hashes of the tuned sources. Thirteen differ; sixteen
are unchanged. The 114 unattempted NeetCode problems remain held out. Performance
rewrites of natural sources are forbidden; correctness corrections require an
explicit record. **Natural qualification is 19/150 and tuned is 22/150, both
unchanged by this round.** Each cohort passes all 1,070 correctness checks at
reference, native O0 and native O2.

## What the copy census established

Part A1 is complete for the application corpus and both NeetCode cohorts, and
**incomplete for the fuzz scope** (see below). Classification is against later
reference-semantic observations by monotonic version ID, with a frozen coverage
ledger; an unledgered event kind aborts the census rather than defaulting to
AVOIDABLE.

The five cliff programs are the **only** programs whose native payload-copy count
differs between the natural and tuned cohorts; the other 24 are identical. Round
20's rewrites removed **94.9% of native copied bytes**, and three programs went
from 10^5–10^6 copies to seven. The cliffs are copy traffic, which confirms the
Part C/D premise and is direct evidence for the natural-code rule.

The REQUIRED/AVOIDABLE split then decides which analysis is needed, and it is not
the obvious one. `validate-binary-search-tree` is 100% AVOIDABLE, so liveness
would fix it; `last-stone-weight` is 99.999% REQUIRED and `balanced-binary-tree`
99.93% REQUIRED, so **liveness cannot touch the two largest cliffs** — only
read-before-write scheduling can. A liveness-only ownership analysis would have
looked like progress while leaving them untouched.

## Preregistration committed before any code

`FORECASTS.md` holds numeric forecasts for steps 4–6. [ADR 043](adr/043.md)
decides configurable limits and depth (defaults 256 MiB and 1024 frames, chosen
from measured headroom), native stack exhaustion becoming a clean E008, an opt-in
`--explain-copies` diagnostic, and an explicit decision **not** to implement
general ownership this round with its eight soundness obligations recorded.

The reserve-as-if-copying rule is explicitly retained: removing it would make a
program's E013 boundary depend on whether an ownership proof fired, so the limit
is raised instead.

A dated addendum to ADR 043 records that the census **falsified** the ADR's own
first reason for declining general ownership. The decision stands on the
remaining reasons; the wrong sentence is corrected by addendum, not edited away.

## Still outstanding

- Steps 4, 5 and 6 are not implemented. Every F4, F5 and F6 forecast remains
  open except F6.1, which forecast that general ownership would not ship and held.
- The fuzz scope of the census reports zero events for both seeds because the
  generated families declare a four-argument `(v,s,s,s)` entry while the harness
  supplies two byte arguments, so each family stops at E006 before executing. A
  helper supplying a Buffer and three Bytes arguments is required.
- 3sum's largest fixture is excluded from the native join in both cohorts, its
  native trace exceeding the 5,000,000-event cap.
- The census measures the native O0 mechanism and makes no claim about which
  copies survive O2.
- The next round's ownership preregistration should target read-before-write
  scheduling first, with trap order as the binding soundness constraint, and must
  carry the variant-shape matrix rule R2 requires.

The reference evaluator already uses heap-allocated function frames; claiming it
needs a recursive-to-iterative rewrite would be incorrect. Native depth checks
remain conditional on bounded instrumentation, so a larger configured depth
cannot be considered safe until native stack exhaustion is handled. The current
64 MiB limit and reserve-as-if-copy rule are unchanged in code.
