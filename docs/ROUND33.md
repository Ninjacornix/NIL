# Round 33 — a retraction, a re-ranking, and no feature

## Unfavourable first

**I was wrong for three rounds about the copy instrument, and the error was
mine, not the compiler's.** ADR 051 addendum A claimed `--explain-copies`
cannot see the `roots == 2` fallback copy, and concluded that every copy figure
in the repository was unreliable. Rounds 30, 31 and 32 repeated it. The truth
is that the flag belongs **before** the subcommand — `nil --profile P
--explain-copies run FILE` — and I had been writing it after `run`, so the CLI
printed `E010 invalid arguments` and exited without executing anything. I read
that silence as a compiler defect and never checked the exit status of the
command whose stderr I was grepping. Withdrawn in ADR 051 addendum C.

**Step 2 was not built.** Nested container values — the feature this round was
called to deliver — does not exist. That is a deliberate stop, recorded in
ADR 053 addendum A *before* any code, because the ADR's own falsification test
fired.

**Nesting's audience is three problems, not six.** Reading the two tree
problems' natural Python shows neither needs nested containers:
`subtree-of-another-tree` runs KMP over flat token sequences and
`serialize-and-deserialize-binary-tree` joins a flat token buffer. Both are
expressible with integer links, available since Round 24. The six-problem
figure was wrong the same way Round 30's seven-problem tokenization figure was
wrong: **a problem that mentions trees is not a problem that needs a tree
type.**

**Five of the fifteen classified problems have no blocker at all.** They are
simply unwritten. Building a type-system feature for three problems ahead of
writing five that need nothing would repeat precisely the mistake the discovery
round was created to stop — which is the third round running where the stated
top blocker shrank once I read the source instead of the label.

**`course-schedule` v2 and `redundant-connection` are unchanged**, as ADR 053
forecast: their copies come from live loop-state roots in the same function,
not from nesting, so nothing this round could have helped them.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| Copy instrument work needed | reporting must be added | **none** — it already reported |
| Nesting's audience | 6 problems | **3** |
| Step 2 shipped | 2–3 problems expressible | **not built** |
| course-schedule v2 / redundant-connection | not helped by nesting | **unchanged**, as forecast |
| Problems written with no new feature | not forecast | **1** (35/35, first attempt) |

## The retaining pattern, recorded

Run correctly, both programs report the copy with site, reason and retaining
root, and the pattern is identical:

```text
course-schedule v1, largest fixture
  {"kind":"replace","bytes":800000,"span":[710,727],
   "retaining_use":[783,784],"reason":"live-root-retention"}
  {"kind":"replace","bytes":800000,"span":[750,760],
   "retaining_use":[783,784],"reason":"live-root-retention"}

redundant-connection, largest fixture
  {"kind":"replace","bytes":800000,"span":[320,335],
   "retaining_use":[220,235],"reason":"live-root-retention"}
```

In both, an update's buffer is still reachable from a **live loop-state root
declared earlier in the same function**, so the update cannot borrow and falls
back to the copying path. This is intraprocedural, which means it is *not* the
interprocedural child-edge problem ADR 051 described — a second correction to
that ADR's framing, and a cheaper problem than previously believed.

## The corrected histogram

| Blocker | Problems |
|---|---:|
| Nested/sequence-valued map values | **3** |
| Worklist or deque under parallel loop state | **3** |
| **No blocker — simply unwritten** | **5** |

Nesting's three: `time-based-key-value-store`, `design-twitter`,
`design-add-and-search-words-data-structure`. The unwritten five:
`subtree-of-another-tree`, `serialize-and-deserialize-binary-tree` (**now
written**), `combination-sum` (unblocked by Round 31's `!parsebuf` fix),
`merge-intervals`, `multiply-strings`.

## What shipped

**A regression test** asserting that a `roots == 2` update copy reports site,
reason and retaining root at O0 and O2, so this cannot be misread a fourth
time. No instrument change was required.

**`serialize-and-deserialize-binary-tree`, 35/35 on the first attempt**, using
only integer links, an explicit stack and Round 31's row join. This is the
evidence for the re-ranking rather than an assertion of it. Timing not taken.

## Gates

`/private/tmp/r33-f1/receipt.json`, every gate exit 0: ci **665**, corpus
**2,815**, both cohorts **1,070/1,070**, fuzz seed 61001 **877 programs** with
0 non-resource divergences and 0 rule-3 failures. Wall was 17,236 s against
~1,528 s of summed gate time, an 11× ratio that indicates heavy external load
rather than a scheduler fault; worth re-checking on a quiet machine.

Natural cohort stands at **33/150** on the overseer's timings, now recorded in
`docs/ROUND32.md` and attributed.

## Commits

```
8462e16 docs(nesting): preregister ADR 053 and retract the copy-instrument defect
4c5b71e test(nesting): lock roots==2 copy reporting so the misreading cannot recur
```

## Next round

Write the four remaining unblocked problems — `subtree-of-another-tree`,
`combination-sum`, `merge-intervals`, `multiply-strings` — before any feature
work. They need no compiler change, and on this round's evidence that is where
qualification actually comes from: the last three rounds moved the cohort
21 → 33 almost entirely by authoring, while two preregistered features
(`!imap`, nesting) produced one user and zero.

Then choose between nesting (3 problems) and sequential loop-step bindings for
worklists (3 problems) on measured cost, not on the order they were discovered.
