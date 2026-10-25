# Round 32 — splitting text

One goal, as instructed. Part 2 (the copy instrument) was not started, which is
correct sequencing rather than another skipped blocker: splitting was the top
item and it is done.

## Unfavourable first

**Splitting unblocks four problems, not seven.** Round 30's histogram credited
tokenization with 7 of 25, and reading the natural Python solution for each
before writing any code showed that three of those —
`time-based-key-value-store`, `design-twitter` and
`design-add-and-search-words` — are blocked *behind* splitting by per-key
growable lists, per-user post lists and a trie of nested dicts. Those need
sequence-valued or nested map values, deferred since ADR 030. Recorded in
addendum B **before** the code. **The honest histogram is splitting 4, nesting
6**, and nesting is now the top blocker.

**So the headline feature of two rounds' planning helps four problems.** It was
still the right call — four is the largest single win available, and the three
nesting-blocked problems were not attempted because attempting them would only
re-derive the paragraph above.

**I measured no timing myself.** `ab_time.py` exists because the same binary
measured 5.080× and 5.735× in two sessions, and the closing audit was running
on this machine, so quoting ratios would have been quoting noise. The timings
in this report were taken **by the overseer**, 15 repetitions on a quiet
machine, and are attributed accordingly; I did not reproduce them.

**No fast gate was run by me**, per the instruction to hold heavy gates until
the audit finished. My verification was per-target `cargo test` and per-task
`verify.py`, which is weaker than a gate receipt; the overseer's closing gate
reported **140/140** checks for these four problems.

**The ADR's spelling preference was overruled by its own measurement rule.** It
said prefer generalizing an existing verb; generalizing `!find` costs 33 cl100k
and a `!parsebuf` mode flag 36, against 27 for a new `!split`. Measurement won.

**Against Python, splitting does not close the gap.** `valid-anagram` is
**+241%** — Python writes `collections.Counter(a)==collections.Counter(b)` and
NIL writes two 256-bucket count loops and a comparison.

## Forecast versus actual

| Measure | Forecast (addendum B) | Actual |
|---|---|---|
| Problems splitting unblocks | 4 (revised from 7) | **4** |
| Of those, qualifying | 3–4 | **4 correct**, timing pending |
| Representation | offset/length pairs, no new type | **as planned** |
| Spelling | generalize an existing verb | **new verb `!split`**, 6–9 tokens cheaper |
| Attempts per problem | ≤3 | **1, 1, 2, 2** |

## Per-problem results

| Problem | Correct | Attempts | NIL tokens | Python | vs Python | × C++ |
|---|---|---:|---:|---:|---:|---|
| valid-anagram | **35/35** | 1 | 99 | 29 | **+241%** | **0.80** |
| distinct-subsequences | **35/35** | 1 | 102 | 75 | **+36%** | **1.81** |
| interleaving-string | **35/35** | 2 | 221 | 169 | **+31%** | **1.17** |
| word-search | **35/35** | 2 | 297 | 173 | **+72%** | **1.11** |

Speed ratios measured by the overseer, 15 repetitions on a quiet machine.
`valid-anagram` is **faster than C++**, which is the second time a 256-bucket
count loop has beaten `std::` containers after `counting-bits` at 0.880×.

Not attempted, blocked behind splitting by nesting: `time-based-key-value-store`,
`design-twitter`, `design-add-and-search-words-data-structure`.

Failures logged: `interleaving-string` attempt 1 died **E012** because a loop
slot shadowed the string whose length bounded the loop — the same trap that
cost an attempt in Round 30, now in the idiom guide. `word-search` attempt 1
died **E007** on an inverted membership test.

## What shipped

`!split(a,seps)` returns a `Buffer` of alternating offset and length into the
input; a field is recovered with the existing `!slice`. **No
sequence-of-byte-strings type was introduced**, so the nested-container work
stays deferred and untouched.

**Empty fields are kept** — the opposite of `!parsebuf`, deliberately, because
Python's `str.split(sep)` keeps them and `str.split()` does not, and the corpus
assumes both behaviours in their respective places. The field count is always
separators + 1. Counted in one pass, allocated once, filled in a second; the
separator argument is a byte **set**, as in `!parsebuf`.

Specified and identical in reference and native at O0 and O2: empty input is
one empty field, all-separator input is n+1 empty fields, leading and trailing
separators produce empty fields, an empty separator set leaves one field, and
multi-byte sets including a high byte work.

**A pleasant discovery worth the idiom guide:** `word-search` needs no explicit
backtracking. Because a set is an immutable value, passing `!add(seen,cell)`
into the recursive calls and discarding it on return *is* the undo. Python
needs `seen.add` / `seen.remove`; NIL needs neither.

**Six new fuzz families**, campaign 871 → **877** programs, 0 non-resource
divergences and 0 rule-3 failures on seed 61001: empty and all-separator input,
separator runs at both ends and interior, multi-byte separator sets, slice
round-trips that must reconstruct the input length, split-join-split agreement,
and a long input that would expose a quadratic pass.

## Qualification

**All four qualify**, on the overseer's timings: 0.80×, 1.11×, 1.17× and 1.81×
C++, with 140/140 checks. The natural cohort goes **29/150 → 33/150**. The
provisional forecast in this section was that 3–4 of 4 would qualify; four did.

## Commits

```
28c807b docs(text): record what splitting actually unblocks before building it
b10f400 feat(text): split bytes on a separator set into offset and length pairs
```

## Next round

1. **Timing for these four, then a fast gate** — the two things this round
   could not do on a busy machine.
2. **Nesting is now the top blocker at 6 problems**: sequence-valued and nested
   map values, which would unblock the three design problems plus the tree
   problems. This is the ADR 030 deferral coming due.
3. **Part 2, the copy instrument** — `redundant-connection` is still 137.5×
   with 99% of samples in `memmove` and nothing reported.
