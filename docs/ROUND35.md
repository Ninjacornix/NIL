# Round 35 — the copy diagnosed, the std pack measured but not built

The goal text named `docs/ROUND34.md` for this report; that file is Round 34's
and already written, so this is `ROUND35.md`.

## Unfavourable first

**Steps 2 and 3 did not happen. No std helper was built and no program was
rewritten.** The round delivered Step 0's diagnosis and Step 1's measurement
and stopped. The +3,865-token gap against Python is exactly where it was.

**Step 0 was time-boxed at about two hours and consumed the round.** That was
the right call on value — the copy now has a three-line reproducer and a named
cause after two rounds of wrong guesses — but it means the headline goal, the
token pack, is untouched.

**My Round 34 probes could not possibly have worked, for a reason I should have
seen.** A **leaf** callee is source-expanded, so it never takes a root. All
three of my probes used leaf callees or element reads, so none of them could
reproduce a copy caused by a *non-leaf* call rooting its argument. I spent a
round concluding "hypothesis disproved" when the hypothesis was right and my
instruments were wrong.

**`subtree-of-another-tree` is a 35.3× cliff I introduced last round and did
not anticipate.** It reports `kind: slice` rather than `replace`, with **713 of
~760 samples in `_platform_memmove`**: my explicit preorder stack does
`!slice(c,0,#c-1)` plus two `!concat`s per node on a retained buffer, so stack
churn is quadratic. Same `live-root-retention` reason, different operation, and
*not* fixed by whatever fixes the update copy.

**The construct attribution double-counts.** A program using both `min/max` and
`sorted` contributes its whole gap to each, so the per-construct figures are
upper bounds, not a partition. The ranking is still usable for choosing work;
the magnitudes are not additive.

## Step 0 — minimal reproducer and exact cause

```text
=#b(!buffer(256,0))
(v):v=@(a,0,0;b<64;a[b:b],b+1,c+c(a,b);a)
(v,i)=@(a,b;a[b]!=b;a,a[b];b)

{"kind":"replace","bytes":2048,"span":[40,45],
 "retaining_use":[52,58],"reason":"live-root-retention"}
```

**Cause:** within one loop step, one state slot updates a buffer in place while
**another slot's expression passes the same buffer to a non-leaf call**. The
call roots its argument for its duration, so the update sees two live roots and
falls back to the copying path.

Reduction table, each row a variant of the real program:

| Variant | Copies |
|---|---:|
| No sibling use | 0 |
| Sibling reads an element `a[b]` | 0 |
| Sibling uses `#a` — whole buffer, no call | 0 |
| Sibling calls a **leaf** `(v,i)=a[b]` | 0 |
| Sibling calls a function **containing a loop** | **1** |
| Update inline or inside a callee | 1 either way |

Candidate 1 from ADR 054 — call-valued index operands — is **ruled out on its
own**: the same union-find shape without the sibling call reports zero copies.
Candidate 2, the nested child edge, is **not needed** to explain these
programs, so this stays intraprocedural, ADR 051's interprocedural framing is
not reinstated, and Round 33's reading stands.

A test pins both halves, including that the leaf-callee variant must **not**
copy, so the fix cannot regress into over-copying.

## Step 1 — where the token gap actually is

52 programs with both a NIL kernel and a Python reference:
**NIL 8,174, Python 4,309, gap +3,865 (+89.7%)**. Full table committed at
`benchmarks/reports/2026-10-14/TOKEN_GAP.md`.

Worst programs: `course-schedule` +329, `merge-intervals` +245,
`spiral-matrix` +213, `subtree-of-another-tree` +207,
`balanced-binary-tree` +192, `last-stone-weight` +182,
`redundant-connection` +162.

Gap attributable to programs using each missing Python construct (upper bounds,
see caveat above):

| Construct | Gap in programs using it |
|---|---:|
| `min`/`max` over a sequence | **+935** |
| `sorted(…, key=…)` | **+564** |
| `.join` | +259 |
| `enumerate`/`zip` | +234 |
| `heapq`/`bisect` | +219 |
| `Counter` / frequency table | +204 |
| `any`/`all` | +196 |
| comprehensions | +182 |

**This contradicts the round's stated expectation.** The goal predicted sorting
with a key would top the list; on measurement `min`/`max` over a sequence
leads it by 1.7×, and it is a far smaller change. The two-key packing trick is
real but it appears in fewer programs than plain `min`/`max` reductions do.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| Copy cause found | candidate 1 (index operands) | **sibling non-leaf call**; candidate 1 ruled out |
| Diagnosis method | mechanical reduction | **worked**, after lookalikes failed twice |
| Top std helper | `sorted(key=…)` | **`min`/`max`**, by 1.7× |
| Helpers built | 5–8 | **0** |
| Token gap closed | not forecast | **0** of +3,865 |

## Gates

None run; the overseer's closing gate was in progress for part of the round and
the remaining work was diagnosis plus measurement. Verification is
`cargo test -p nil-llvm --test read_replay` (**25 passed**) and the per-task
`verify.py` runs from Round 34.

Natural cohort stands at **36/150** on the overseer's timings, recorded in
`docs/ROUND34.md`.

## Commits

```
bf04b4d test(std): find the live-root-retention copy's cause by mechanical reduction
f0461d7 chore: advance the benchmarks pointer for the token gap analysis
```

## Next round

1. **`min`/`max` over a sequence first**, not sorting — it is the largest
   measured saving and the smallest change. Then `sorted` with a key,
   `any`/`all`, and a frequency helper.
2. **Fix the update copy** now that it has a reproducer and a test: release the
   sibling call's root when the called function provably only reads its
   argument. The borrowing contract from ADR 035's plugin work already proves
   read-only parameters, so the machinery may exist.
3. **`subtree-of-another-tree`'s stack churn is a separate fix** and the
   cheaper one: a push/pop that does not copy a retained buffer, or writing
   that program against a preallocated stack with an index.
