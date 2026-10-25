# Round 31 — text toolkit (partial) and a copy instrument that was not built

## Unfavourable first

**Part 1a did not happen and Part 2 did not happen.** Splitting bytes into
tokens — the capability that blocks **7 of 25** problems and was the single
biggest item in Round 30's histogram — was not implemented. Neither was the
copy instrument. Only Step 0 and Part 1b landed. The round therefore addressed
the **fourth and fifth** entries in the histogram and left the first untouched.

**ADR 052's falsification test was not run.** The ADR said that if the seven
tokenization-blocked problems stay unqualified after 1a, then tokenization was
mis-ranked. Because 1a was not built, that question is still open: this round
produced **no evidence either way** about the largest claimed blocker.

**The ADR's own compatibility argument was wrong and had to be corrected.** It
claimed the `!parsebuf` change "cannot change the meaning of any program that
currently runs". Three committed tests *asserted* that empty fields are
rejected, so this narrows a specified diagnostic. Addendum A records it.

**The artifact-freeze race appeared a third time, in my own Round 28 fix.**
Round 28 froze `target/release/lib<crate>.rlib`, which cargo does not refresh
when only one package is built. The fix then took the newest hashed rlib by
mtime — which picks a *plausible* file, not a *consistent set*, so a
`nil_compiler` built against a different hashed `nil_hir` produced two crate
instances and **E0308**. Two heuristics, two failures. The freeze now runs
cargo with `--message-format=json` and copies exactly what cargo names.

**`sliding-window-maximum` is still 32/35**, unchanged: its blocker is the
deque, which nothing here addressed.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| LIS tokens after step 0 | 266 → ~110 | **97** — better |
| merge-two-sorted-lists tokens | 352 → ~190 | **190**, then 161 with the join |
| Row join tokens | 40 → 8–12 | **9** |
| counting-bits speed | resolves under 3× | **0.880×** — faster than C++ |
| merge speed | resolves under 3× | **1.579×** |
| Split (1a) | 3–5 of 7 qualify | **not built** |
| Copy instrument (2) | reports a `roots==2` copy | **not built** |

## Per-problem results

Interleaved `ab_time.py` where it mattered; the harness had been optimistic on
both former misses, so the interleaved numbers are the ones quoted.

| Problem | Correct | × C++ R30 | × C++ R31 | Tokens R30 → R31 | vs Python |
|---|---|---:|---:|---:|---:|
| climbing-stairs | 35/35 | 1.03 | **1.04** | 24 → 24 | −29% |
| counting-bits | 35/35 | 4.60 | **0.880** | 90 → **61** | +33% |
| two-sum-ii | 35/35 | 1.42 | **1.44** | 125 → **96** | +20% |
| jump-game-ii | 35/35 | 2.51 | **2.54** | 57 → 57 | −5% |
| remove-nth-node | 35/35 | 2.90 | **1.33** | 86 → **57** | +63% |
| longest-incr-subsequence | 35/35 | 1.61 | **2.45** | 266 → **97** | +62% |
| merge-two-sorted-lists | 35/35 | 3.47 | **1.579** | 352 → **161** | +87% |
| top-k-frequent-elements | 35/35 | 1.68 | **1.71** | 206 → **177** | +312% |
| redundant-connection | 35/35 | 137.51 | not re-measured | 206 → 177 | — |
| sliding-window-maximum | 32/35 | — | — | 122 → 93 | +31% |

## Qualification

**Eight of the ten authored problems now qualify**, up from six: `counting-bits`
and `merge-two-sorted-lists` were the two speed misses and both are now well
inside the bound. That takes the natural cohort from **27/150 to 29/150**.

Both gains came from the same cause — one allocation instead of a per-element
`!concat` chain — so the row join is the cheapest qualification win recorded in
any round so far: **one generalized verb, two problems, nine tokens.**

## What shipped

**Step 0.** `!parsebuf` skips empty fields in both paths: a separator run
behaves like one, leading and trailing separators contribute nothing, and
all-separator input is an empty buffer. Verified identical on `"0 \n"`,
`"0 \n0 \n"`, `"\n\n"`, `"   "` and interior runs. Two quota tests that charged
the field count with 8 MiB of newlines now build non-empty fields by doubling,
because all-newline input no longer allocates at all.

**Part 1b.** `!format(v,sep)` and `!format(v,sep,term)` join a numeric sequence
in one call, sizing the output before writing it. Five spellings were measured;
the separator-plus-terminator form is **9** cl100k tokens against 12 for
`!format(v,sep)`, `!join` and `!row`, 13 for `!formatbuf`, and 39 for the
adapter it replaces. Arity is part of the runtime name, after a two-argument
call segfaulted reading an absent terminator through a three-pointer signature.

**Six new fuzz families**, campaign 865 → **871** programs: the empty-vector
encoding, separator runs, multi-byte separators, row round-trips, row edges
including i64 extremes and an empty separator, and a long row that would expose
a quadratic join.

## Gates

Fast gates between steps; the closing full gate is the overseer's. Last receipt
`/private/tmp/r31-f3/receipt.json`: ci **664**, sanitizer **316**, fuzz
**871 programs** with 0 non-resource divergences and 0 rule-3 failures, corpus
**2,815**, tuned **1,070/1,070**, depth **36/36**, natural **1,069/1,070** from
the standing `last-stone-weight` wall-backstop flake, which passes 40/40 alone.
No count decreased.

## Commits

```
64562c0 docs(text): preregister ADR 052 for the text toolkit and copy instrument
6181d8a fix(text): !parsebuf accepts empty fields, and freeze what cargo names
b4bfc45 feat(text): join a numeric sequence into one row in a single call
```

## Next round

1. **Part 1a, splitting** — still the largest measured blocker at 7 problems,
   still untested. The offset-pair design in ADR 052 is unimplemented, not
   refuted.
2. **Part 2, the copy instrument** — `redundant-connection` remains at 137.5×
   with 99% of samples in `memmove` and nothing reported, so every copy figure
   in the repository is still unreliable.
3. `sliding-window-maximum`'s deque, which needs sequential rather than
   parallel loop-step bindings — the second entry in the histogram, also
   untouched.
