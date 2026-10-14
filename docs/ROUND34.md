# Round 34 — four problems written, one copy not fixed

## Unfavourable first

**Part 2 did not land. No compiler code was written at all.** The
live-root-retention copy is not fixed, `redundant-connection` is still 137.5×
C++ and `course-schedule` v2 still does not run. I stopped at diagnosis
because the diagnosis failed.

**My hypothesis was wrong, or at least unconfirmed.** I predicted the second
root comes from a sibling loop-state expression reading the same buffer in the
same step. I wrote three minimal reproductions and **none of them copied**: a
loop-carried buffer updated inside a callee, the same with a sibling slot
reading an element, and the same with a sibling slot calling a function on the
whole buffer. All three reported zero copy hints. So the small cases that look
like `redundant-connection` do not behave like it, and I do not know why.

**I declined to record the forecast the goal asked for.** Promising
`course-schedule` and `redundant-connection` at ≤3× C++ would have been a
number with nothing behind it, and the last three rounds were damaged by
exactly that — committing to a ranking or a cause before reading the evidence.
ADR 054 records the candidates and no mechanism.

**One of those candidates would partly undo Round 33's claim.** If the
retaining root turns out to be a nested child edge (`b.counts`, `b.tail` in
`course-schedule`), then this is the interprocedural child-edge problem ADR 051
described and my "intraprocedural, therefore cheaper than believed" conclusion
was wrong. I flag that against myself in the ADR.

**The authored problems are token-expensive.** `combination-sum` is **+850%**
against Python's ten-token one-liner and `merge-intervals` **+366%**.

**No gate was run**, per the instruction to hold heavy jobs while the closing
audit runs. Verification is per-task `verify.py` only.

## Part 1 — four problems, no compiler change

All four pass **35/35** across reference, O0 and O2.

| Problem | Correct | Attempts | NIL tokens | Python | vs Python |
|---|---|---:|---:|---:|---:|
| Problem | Correct | Attempts | NIL tokens | Python | vs Python | × C++ |
|---|---|---:|---:|---:|---:|---:|
| multiply-strings | **35/35** | 1 | 204 | 116 | +76% | **0.98** |
| merge-intervals | **35/35** | 1 | 312 | 67 | +366% | **0.92** |
| combination-sum | **35/35** | 2 | 95 | 10 | **+850%** | **2.46** |
| subtree-of-another-tree | **35/35** | 1 | 369 | 162 | +128% | **35.3 CLIFF** |

Speed measured by the overseer, 15 repetitions. Three qualify, taking the
natural cohort to **36/150**. `subtree-of-another-tree` is a **35.3× cliff**
(766 ms against 22 ms) and does **not** qualify — my KMP-over-token-buffers
formulation hits a performance wall I did not anticipate, and it is now a third
data point for the copy investigation alongside `redundant-connection` and
`course-schedule`.

Failure logged: `combination-sum` attempt 1 died **E004 unknown function 4**
because the recursive call used the next function letter rather than its own —
a function with four parameters is still called by its own declaration letter,
and `d(…)` inside a four-parameter function is a call while bare `d` is the
fourth parameter.

Nothing here needed a feature. `subtree-of-another-tree` runs KMP over flat
preorder token buffers with a sentinel for null, using integer links available
since Round 24 — which is the concrete demonstration that Round 33's
re-ranking was right to call it unblocked rather than nesting-blocked.
`merge-intervals` and `combination-sum` lean on the two-key packing trick,
now in the idiom guide, because there is no comparator sort.

**Measured:** three of four qualify — `merge-intervals` 0.92×,
`multiply-strings` 0.98×, `combination-sum` 2.46× — taking the cohort
**33 → 36/150**. `subtree-of-another-tree` misses at **35.3×**.

## Part 2 — what is established and what is not

Established, with the flag passed correctly:

```text
redundant-connection  replace 800000 bytes  span [320,335] = "[g(a,b):g(a,c)]"
                      retaining_use [220,235] = "g(b,a[2+c*2+1])"
course-schedule v1    replace 800000 bytes  span [710,727] = "[c:b.counts[c]-1]"
                      replace 800000 bytes  span [750,760] = "[b.tail:c]"
                      retaining_use [783,784] = "?"
```

Not established: the cause. Three probes ruled out the obvious story. The
remaining candidates, in the order worth testing next:

1. **The update's own index operands are calls on the same buffer.**
   `a[g(a,b):g(a,c)]` roots `a` twice inside the update expression itself, and
   the reported update span *is* that expression. This is now the leading
   candidate and the probes did not cover it.
2. **A nested child edge** — `course-schedule` updates `b.counts` and `b.tail`
   through a parent record, which no probe modelled.
3. **Scale** — probes used 4,096 elements and 64 iterations against 20,000
   elements and tens of thousands of iterations; the reuse proof may be
   capacity- or collection-dependent.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| Four problems correct | not forecast | **4/4**, 1, 1, 2, 1 attempts |
| Compiler change needed for them | none | **none** |
| Part 2 diagnosis | sibling-read retention | **disproved by three probes** |
| Part 2 mechanism | general release/transfer | **not written** |
| course-schedule / redundant-connection | ≤3× C++ | **unchanged**; forecast declined |

## Commits

```
5aa9dcd bench(loopstate): write the four remaining unblocked discovery problems
c2ae798 chore: advance the benchmarks pointer for the four authored problems
fbb3e24 docs(loopstate): ADR 054 records an incomplete diagnosis and no mechanism
```

## Next round

1. **Test candidate 1 first** — build a probe where the update's index operands
   are calls taking the same buffer. It is one small program and it decides
   whether this is a local fix or the child-edge work.
2. If candidate 2 turns out to be the cause, reinstate ADR 051's framing and
   correct Round 33's report rather than leaving the two inconsistent.
3. The remaining unwritten discovery problems are the three nesting-blocked
   design problems and the three worklist problems; on this round's evidence
   authoring is still the better-yielding activity, so the worklist three are
   worth attempting before the nesting feature is built.
