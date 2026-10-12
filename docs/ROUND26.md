# Round 26 — data types checkpoint

## Unfavourable results first

**1. The feature works, cuts tokens 63%, and still cannot qualify — because
native recursion is capped at 256 frames.** The qualification harness
(`benchmarks/helpers/core_check.rs:59`) builds every candidate with
`call_depth: 256`. The frozen `largest` fixture for both tree problems is a
**100,000-node chain of depth 100,000**. So the natural recursive formulation
reports `E008 call depth limit exceeded` at O0 and O2 on that one fixture:

```
balanced-binary-tree        largest O0 exit 1 'E008 @148..161 call depth limit exceeded'
balanced-binary-tree        largest O2 exit 1 'E008 @148..161 call depth limit exceeded'
validate-binary-search-tree largest O0 exit 1 'E008 @176..189 call depth limit exceeded'
validate-binary-search-tree largest O2 exit 1 'E008 @176..189 call depth limit exceeded'
```

Six of seven fixtures pass at reference, O0 and O2 for both programs; the
seventh makes each `resource-inconclusive`, so **neither v2 program qualifies**.
This is the round's central finding, and it is more important than the token
number: *this* is why v1 was written with explicit heap stacks and offset
arithmetic in the first place. Recursive types remove the encoding but not the
reason for it. Until deep recursion is supported — a growable stack, a raised
depth contract, or lowering recursion to an explicit stack — the 63% token
saving is unavailable on any input deeper than 256.

**2. None of the seven validation-blocked problems is made expressible by sum
or recursive types.** Their own recorded analyses in `STATUS.json` say so:

| Blocked problem | Recorded analysis | Does B1 help? |
|---|---|---|
| lru-cache | "replace candidate with indexed doubly linked list + lookup. No core impossibility established." | **No** — a doubly linked list is cyclic, and ADR 050's acyclic-by-construction guarantee forbids it |
| implement-trie-prefix-tree | "Native O2 succeeds; O0/reference exceed deadline." | No — needs map-valued sums (B3) |
| word-search-ii | "Per-word DFS rather than shared trie traversal; algorithm class differs." | No — needs B3 |
| n-queens | "both native paths succeed, reference exceeds deadline" | No — evaluator speed |
| course-schedule-ii | "Scans all vertices…; replace candidate with a min heap." | No — algorithm class |
| network-delay-time | "Dense vertex selection…; algorithm class differs." | No — algorithm class |
| cheapest-flights-within-k-stops | "both native paths succeed, reference exceeds deadline." | No — evaluator speed |

Four are blocked by reference-evaluator deadlines or algorithm class, which
types cannot touch. Two need B3. One, `lru-cache`, is actively *excluded* by
this ADR's acyclicity guarantee. The goal's premise that these seven become
expressible is not borne out. **Newly expressible problems: 0.**

**3. Sum types made one program longer.** `validate-binary-search-tree` v1 is a
single dense in-order loop, already compact; its v2 needs two declarations and
nested matching and comes out **+2.3% on the kernel cut** and 2.5× C++ instead
of 2.0×. The saving appears only where v1 had to simulate recursion.

**4. Consume-and-rebuild recursion still misses 3× C++ after two
optimizations, and the residue is structural.** Repeated BST insertion, 60,000
keys, O2, 15 repetitions:

| Insert/rebuild | median | vs C++ |
|---|---:|---:|
| as first landed | 237.18 ms | 9.786× |
| + retired-node reissue | 152.78 ms | 6.198× |
| + borrowed match bindings | **129.52 ms** | **5.132×** |
| C++ mutable BST | 25.24 ms | — |

Both steps are justified by the ADR's own rule, since the bound was missed.
Neither is Perceus reuse: the first recycles storage without any aliasing
claim, the second removes two reference operations per level by observing that
a field binding is read-only and its parent outlives the function.

**The remaining 5.13× is not an allocator problem.** NIL's `insert` rebuilds
the path, writing a node per level, where C++ mutates one pointer per level.
That is O(log n) writes against O(1), and no representation change fixes it —
only in-place mutation does, which is what Perceus reuse is. Closing it
therefore needs **field-level reuse specialization**: consume the matched node,
reuse its storage, and write *only the slot that changed*. Reuse that still
rewrites every slot would save roughly two reference operations and two word
writes per level out of a 5× gap, so it would likely reach 3–4× rather than
parity.

**A3 is therefore not implemented, deliberately.** The two cheap levers were
taken and measured; the third needs consuming matches with owned bindings,
which directly trades away the borrowed-binding win above, so the two are
alternatives rather than additive. That interaction was not visible when the
addendum was written and is the main new fact here.

**5. Multi-slot inline sums cannot nest.** Every payload slot is carried as one
i64, so a non-recursive sum inside another sum is refused with E010. Nesting
through a *boxed* sum works, because that is a pointer. Multi-slot packing is
the follow-up.

**6. Sequences of sequences already existed, and using them is still slower.**
`v[v]` is not new capability: a wrapper record has supplied nested sequences
since ADR 030, and `course-schedule` v2 was rewritten that way with **no new
feature at all**. It is 26 kernel tokens shorter and was **3.75× slower**. Half
that gap turned out to be a collector defect rather than anything about nested
types — see the arena-sweep section below — and fixing it leaves v2 at
**290.681× C++ against v1's 205.293×, so 1.42× slower**. Buying 10 tokens for a
1.42× regression is still a poor trade, and the copy-on-append remains
untouched, so **addendum B's refusal of the spelling stands** on the new
numbers. The genuinely missing capability is map values holding sequences,
records or sums, which stays deferred.

**7. B2 is half delivered: users can drop sentinels, APIs cannot.** A
user-declared `sum O(None,Some(i))` is sentinel-free and, after A1, emits
**zero** allocation calls — verified on a map-lookup program that returns 30
with `0` `call ptr @nil_node_make` in its module, and used for real by
`validate-binary-search-tree` v2's `sum B(Free,Bound(i))`. But **no intrinsic
returns an `Option`**, so a lookup must spell `!has(a,b)?Some(!get(a,b)):None`
and probe the map twice, and `!get` keeps its E019 trap. The built-in
monomorphized `Option` that ADR 050 §5 specified for intrinsic results is not
implemented, so the sentinel is gone from user code but not from the core API.

The second probe turns out to be **free**: against a trapping `!get` over 2,000
present keys both forms return 4,002,000 at **3.71 ms versus 3.72 ms (−0.3%)**,
because key construction dominates the probe. So the remaining gap is
ergonomic, not performance — and the trapping form is not merely uglier, it
**fails with E019** on the first absent key where the Option form returns a
default. Qualification still stands at natural 21/150 and tuned 23/150.

## Addendum results (A1, A2)

**A1 — inline layout removes a 25× penalty.** A sum is boxed only when it is
recursive or holds a boxed sum; everything else is a tagged aggregate. An
`Option` build-and-match program now emits **zero** `nil_node_make` calls in
the whole module. Measured on a 20,000,000-iteration lookup loop, O2, 15
repetitions, against a control forced to box by adding a recursive variant:

| Option lookup loop | min | median | max | vs C++ |
|---|---:|---:|---:|---:|
| **inline (A1)** | 12.87 | **12.98 ms** | 15.60 | **1.022×** |
| boxed control | 323.04 | 324.73 ms | 378.00 | 25.578× |
| C++ `std::optional` | 12.58 | 12.70 ms | 13.21 | — |

So a sentinel-free `Option` is at **C++ parity**, where the boxed
representation the original ADR specified would have been **25.03× slower** and
unusable. This is the single clearest vindication of the overseer correction.

**A2 — nodes are reclaimed.** Boxed nodes are reference counted; destruction
threads an intrusive worklist through the nodes being destroyed, so a 10⁶-node
drop allocates nothing and does not recurse. Nodes sit on no list, so a
counting defect surfaces as a real leak rather than being hidden by a sweep.
The former published limit is gone: a two-million-iteration churn loop and
forty rounds of five-thousand-node lists now complete where they previously
exhausted the allocation guard. Validated by 282 sanitizer tests with no ASan
or UBSan findings, and two full fuzz seeds with zero divergences.

## Measured results

### Per-program table (kernel cut, cl100k; O2, 15 repetitions)

Timings are on a 100,000-node **balanced** tree (depth 17), because the frozen
`largest` fixture is a depth-100,000 chain that the 256-frame cap rejects. One
machine, whole-process wall time.

| Program | v1 tok | **v2 tok** | Python tok | v2 vs v1 | v2 vs Python | v1 O2 median | **v2 O2 median** | C++ median | **v2 / C++** |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| balanced-binary-tree | 292 | **107** | 164 | **−63.4%** | **−34.8%** | 28.38 ms | **26.33 ms** | 12.36 ms | **2.130×** |
| validate-binary-search-tree | 130 | **133** | 95 | +2.3% | +40.0% | 23.83 ms | **30.33 ms** | 12.06 ms | **2.516×** |
| course-schedule | 350 | **324** | 162 | −7.4% | +100.0% | 4,539.73 ms | **6,427.97 ms** | 22.11 ms | **290.681×** |

Only one of the three is a clear win. `course-schedule`'s timings are on the
largest frozen fixture, the other two on a 100,000-node balanced tree, because
their largest fixture is a depth-100,000 chain the 256-frame cap rejects.

Full-program (not kernel) cl100k: balanced-binary-tree 379 → **138** (−63.6%),
Python 356; validate-binary-search-tree 153 → **164** (+7.2%), Python 291.

Min/median/max over 15 repetitions, balanced-binary-tree: v1 28.19 / 28.38 /
206.75 ms, v2 25.97 / 26.33 / 209.81 ms, C++ 12.13 / 12.36 / 190.32 ms. The
maxima are first-run warmup outliers.

On `balanced-binary-tree` v2 is **both 63% shorter and slightly faster than
v1**, and inside the 3× bound. The ADR forecast of 100–120 kernel tokens was
met exactly at 107; the forecast that the witness might miss 3× C++ was not
borne out on a depth-feasible input.

### Spelling by measurement

Candidates measured with tiktoken 0.12.0 on real program text. Both tokenizers
agreed within one token on every candidate, so only cl100k is tabulated.

| Declaration | cl100k | |
|---|---:|---|
| `sum Tree(Node(i,Tree,Tree),Leaf)` | 11 | **chosen** |
| `enum …` / `type …` / `sum Tree=…\|Leaf` / `record Tree(…\|Leaf)` | 11 | |
| `Tree=Node(i,Tree,Tree)\|Leaf` | 10 | rejected: `Name=` is ambiguous with a positional function |

| Match (over a complete four-function program, two occurrences) | cl100k | |
|---|---:|---|
| `?a(N(_,l,r):X,L:Y)` | 156 | **chosen** |
| `a?(N(_,l,r):X,L:Y)` | 156 | rejected: collides with the ternary `a?(x):(y)` |
| `match a(…)` / `?a N(…):X;L:Y` / `?a(N(…)=>X,L=>Y)` | 157 | |
| `?a{N(…):X,L:Y}` | 158 | rejected: `{}` is record update |

| Option | cl100k | |
|---|---:|---|
| `?!get(a,b)(?v:v,?:0)` — `?T` sugar | 17 | rejected: overloads `?` a third time |
| `?!get(a,b)(Some(v):v,None:0)` — built-in monomorphized | 18 | **chosen** |
| user-declared `sum OptI(Some(i),None)` + match | 26 | free under B1, and used by the BST validator's `sum B(Free,Bound(i))` |

| Nested containers (`course-schedule` adjacency build) | cl100k |
|---|---:|
| v1 `record Adj(head:v,next:v,to:v)` index links | 88 |
| `v[v]` sequence of sequences | **64** |
| `map[v]` keyed adjacency | 81 |

**A grammar ambiguity the token measurement did not catch:** `?a(…)` collides
with a call. Resolved by making a `(` end the match head, so the scrutinee is a
primary without a call suffix; a call must be parenthesized. Costs nothing in
practice — every scrutinee written this round is a parameter or local.

## What landed

**B1 frontend and reference semantics** (`a04291c`). Declarations form one
group, so a sum may refer to itself or forward to a later one. Records stay
acyclic and earlier-only, being inline aggregates with no indirection, so
recursion goes through a sum payload. Sums are checked for inhabitation, so
`sum X(OnlyX(X))` is rejected. `match` desugars to a tag read plus nested lazy
`If` regions, so arm laziness, rooting and liveness reuse existing machinery and
HIR gains no control form. New exact Check diagnostics **E025** (non-exhaustive,
naming uncovered variants) and **E026** (unreachable arm).

**A soundness hole found and closed.** Variant names resolve across the whole
group, so a match whose arms belong to another sum would have silently selected
the wrong variant whenever payload types agreed — and always when every arm is
nullary. `SumTag`/`SumField` now carry their expected sum type and the validator
requires the operand to match, making it an exact E007.

**B1 native lowering** (`7a7e125`). Nullary variants are unboxed as
`(tag<<3)|1`: heap nodes are 8-aligned, so an odd slot is never a node and
`None`/`Leaf`/`Nil` never allocate. Payload variants are heap nodes on their own
list. `Type::Sum` is deliberately **not** `is_dynamic`: dynamic means
root-managed arena storage, and rooting a list head would retain every child
transitively, O(n) per root store and recursive. Classifying sums as dynamic
made the emitter call `nil_root_store` through the unboxed sentinel and
segfault — found and fixed during this round. Native payloads are one i64 slot,
so wider and dynamic payloads are refused with E010 before emission, and a sum
cannot cross the native entry boundary.

**B4 complete, and it was a specification defect** (`0eb1424`). Record buffers
already reported E014 for an invalid length; `EXPR_V5.md` documented E013 and
never matched. Corrected, and pinned on the reference path and natively at O0
and O2.

**Cohort natural-v2** in a new directory, v1 untouched: `balanced-binary-tree`
and `validate-binary-search-tree`, each with a kernel cut. Both differentially
match v1 on all seven frozen fixtures through the file entry; both are
`resource-inconclusive` under the harness for the depth reason above.
`reverse-linked-list` was **not** rewritten: its `a[2+2*c]` offsets parse an
index-linked *input format* that every language reference also parses, so it is
not a workaround. `course-schedule` needs B3.

## Status by step

| Step | State |
|---|---|
| ADR 050 + addendum | both committed before the code they govern (`c945eaa`, `390f64f`) |
| B1 sums + recursive + match | frontend, reference and native land; reclamation and inline layout land; **unique-node reuse owed** |
| B2 Option | **half**: user-declared Option is sentinel-free, allocation-free and costs −0.3% against a trapping `!get`, but no intrinsic returns one, so the gap is ergonomic |
| B3 nested containers | `v[v]` **refused on measurement** (addendum B); map-valued sequences deferred |
| B4 record-buffer E014 | complete |
| natural-v2 cohort | **3** programs rewritten and verified against v1 on every frozen fixture; none qualifies — two on the 256-frame cap, one on the reference deadline |
| Fuzz families | **22 added**, all exercised by both seeds |
| A1 inline non-recursive sums | complete, at C++ parity |
| A2 node reclamation | complete |
| A3 owned arguments + node reuse | **not implemented**; two cheaper levers taken instead, 9.79× → 5.13× C++, residue shown to be structural |

## Gates

Run at `851556d`, the whole ten-gate sequence in order. Every commit after it
is documentation only, and `./scripts/ci.sh` was re-run at the final commit to
confirm 634 tests still pass.

| Gate | Result | Required |
|---|---|---|
| `./scripts/ci.sh` | **634 passed**, 0 warnings | ≥596 |
| `./scripts/ci.sh release` | **634 passed**, 0 warnings | ≥596 |
| `./scripts/sanitize.sh` | **284 passed**, no ASan/UBSan findings | ≥274 |
| `fuzz-v5.sh --seed 61001` | **843** programs, **1,798** builds, **0 non-resource divergences, rule_3 0**, 36 E008 inconclusive, all 22 sum families exercised | 0 / 0 |
| `fuzz-v5.sh --seed 83003` | **843** programs, **1,798** builds, **0 non-resource divergences, rule_3 0**, 36 E008 inconclusive, all 22 sum families exercised | 0 / 0 |
| application corpus | 2,815 checks, 0 inconclusive | 2,815 |
| corpus `--group original` | **exactly 610** | 610 |
| NeetCode `--verified-only` | 1,070 / 1,070 | 1,070 |
| NeetCode natural cohort (v1) | 1,070 / 1,070 | 1,070 |
| `audit_depth.py` | **36/36**, 0 failed | 36/36 |

The sanitizer script gained `--test sums`, which with the two memory-guard
regression tests is why it rises 274 → 284; the node allocations and the
collection gate are clean under ASan and UBSan. **LeakSanitizer is unavailable
on macOS** (`detect_leaks=0`), so leaks are not checked on this machine — an
accepted limit, and relevant because nodes are not reclaimed during execution.
Every frozen corpus count is unchanged, which is the proof that the new syntax
altered no existing program. No frozen input was edited.

## Round rules, verified

| Rule | Evidence |
|---|---|
| ADRs committed before their code | `c945eaa` precedes the first code commit `a04291c`; addendum A `390f64f` precedes `b388b9c`/`e89b46d`; addendum C `e530951` precedes `7f90064` |
| Later ADR edits append only | all five commits touching `docs/adr/050.md` show **0 deletions** (`+287 −0`, `+81 −0`, `+52 −0`, `+57 −0`, `+15 −0`) |
| Frozen inputs untouched | `git diff 7af71c6..HEAD` matches nothing under `core-generalize-study/natural/`, `tasks.json`, `goldens`, `fixtures.py` or `application-v5/`; the submodule diff adds only `natural-v2/` |
| Earlier profiles unchanged | no diff touches `EXPR_V0`–`EXPR_V4` |
| New syntax changes no existing program | corpus 2,815 and original exactly 610, both NeetCode cohorts 1,070 — every count identical to the branch point |
| Never pushed | `feat/core-types` has no upstream (`git rev-parse @{u}` fails) |
| No shape recognizers | the two optimizations are a size-bucketed free list and a collection trigger; neither inspects program shape |

## Commits

```
c945eaa docs(types): preregister sum, recursive, match, Option and nested containers
a04291c feat(types): add sum types, recursive declarations and pattern matching
0eb1424 fix(docs): record buffers report E014 for an invalid length, not E013
65508e6 docs(types): record the Round 26 checkpoint, gates and open native work
8e6b192 test(types): pin that deep-list depth is bounded by the call guard
266d9eb test(types): drive the depth assertion with an explicit limit
7a7e125 feat(types): lower sums natively with unboxed nullary variants
fc61913 docs(types): report the native sum result and the 256-frame depth blocker
c0b2719 test(types): add 22 differential fuzz families for sums
390f64f docs(types): append ADR 050 addendum on inline sums, reclamation and owned arguments
b388b9c feat(types): reclaim sum nodes by reference counting with a non-recursive drop
e89b46d feat(types): lay out non-recursive sums inline so Some(x) never allocates
226a274 docs(types): record the A1 and A2 results and the insert/rebuild baseline
7117e4d docs(types): state the A3 implementation plan in concrete terms
61057cb perf(types): reissue retired sum nodes from size-bucketed lists
26010b9 perf(types): borrow match bindings instead of retaining them
6f0d387 docs(types): record the rebuild optimization chain and why the residue is structural
e595005 docs(types): append ADR 050 addendum B refusing the v[v] spelling on measurement
e4c9959 docs(types): add course-schedule to the per-program table and record the B3 finding
9aff725 docs(types): diagnose the nested-container copy to a two-line runtime condition
9ec1d84 docs(types): refresh the gates table and commit list
ab50988 docs(types): measure nested-container scaling in both nodes and edges
e530951 docs(types): append ADR 050 addendum C on entry-spaced arena collection
7f90064 perf(memory): space arena collection in entries as well as bytes
0617ac8 docs(types): correct the nested-container diagnosis and record the collector fix
c6ee2c0 docs(types): refresh gates, controls and commit list
2790346 test(memory): pin results and the guard across the collection change
aa5a0c8 docs(types): finalize gates and record that the collector gain is measured, not synthetic
29e89c8 docs(types): state B2 precisely as half delivered
851556d docs: lead the README with the Round 26 result
094f138 docs(types): record the verified round rules and the final commit list
3160da8 docs(types): measure the sentinel-free Option against a trapping lookup
692a77c docs(types): correct the gates table to the final verified counts
```

Submodule `exp/core-types`:

```
8824f0c bench(types): add the natural-v2 cohort for sum and recursive types
f4147d8 bench(types): add course-schedule v2 with wrapper-record adjacency
```

Branches `feat/core-types` (from `origin/master` 7af71c6) and submodule
`exp/core-types` (from 098f8f4). Nothing pushed; nothing amended.

## The nested-container cost, diagnosed and half of it fixed

Worth recording because the first diagnosis was wrong and instrumentation
settled it.

`--explain-copies` reports the `concat` copying with
`reason: live-root-retention`, retained by the very row update that consumes
it. The static side is already right — the emitter selects `@nil_concat_unique`
at all three adjacency sites — and it is the **runtime** test that fails:
in-place append needs `a->roots == 1`, and a projected row child has two roots,
one from the enclosing row and one from the projection. That copy is real and
**still unfixed**.

But it was not the dominant cost. Scaling showed the nested version growing
with the *node* count even though the work does not, which a per-append copy
cannot explain, and the hypothesis recorded earlier was that `nil_child_roots`
traverses every row on each root transition. **Instrumentation refuted that**:

| adjacency build, 20,000 nodes | child rows visited | collections | allocations swept |
|---|---:|---:|---:|
| v1, integer links | 0 | 32,596 | 273,296 |
| v2, nested | 160,000 | 9,272 | **116,586,967** |

Child rooting is eight calls in total. The cost is the **arena sweep**:
`nil_collect` walks the whole allocation list, and a nested collection holds
one allocation per row, so each sweep is O(rows). v1 collects more often and
stays cheap because its list is short; v2 pays about 12,500 visits per sweep.

Addendum C fixes that by comparing sweep cost against yield — memory pressure
always collects, a short list still collects on bytes alone, and a long list
additionally requires geometric growth in the entry count. `course-schedule` v2
falls **16,487 → 6,572 ms** (a 2.51× improvement, interleaved against a clean
build of the preceding commit, outputs matching) while v1 is unchanged, and
interleaved application controls move within noise: scan +0.6%, transform
+1.8%, append −2.2%.

The improvement **could not be reduced to a synthetic case** — two minimal
nested row-append loops show 1.00–1.51×, i.e. noise — so the trigger is more
specific than "nested collection with row appends" and is not fully
characterised. Collection timing is implementation defined and not observable
through program results, so the regression tests added pin what a collection
change can actually get wrong (results unchanged, guard still reports E013)
and the performance claim rests on the cohort measurement.

Two lessons worth carrying: the first version of the gate lacked the small-list
exemption and cost the transform control 20%, and the sequential measurement
that found it exaggerated the effect — only interleaved runs separated signal
from drift.

**What remains** is the copy-on-append above. Releasing the parent's root for a
child being read-modify-written back into the same row is the narrow fix, and
it is a dataflow proof about one buffer and one index rather than a shape
recognizer.

## A3 next step, in concrete terms

Reaching the 3× bound on rebuild needs field-level reuse specialization. The
pieces must land together or not at all:

1. **Transfer at a last use.** A boxed-sum argument at its last use must be
   taken out of the caller's frame slot, not merely left unreleased: the slot
   physically still points at the node, so `nil_node_frame_release` would
   double free. Needs `nil_node_slot_take(void **slot)`, which returns the
   value and nulls the slot, plus a value→owning-slot map in the emitter and
   the existing per-value last-use data. A non-last use retains instead.
2. **Owned sum parameters.** At function entry each boxed-sum parameter is
   placed in a frame slot, exactly as `own_node` does for instructions, so the
   callee releases what the caller handed over.
3. **A reuse token.** A match arm that is the last use of its scrutinee must,
   after binding fields with retains, hand the node to
   `nil_node_take(p)` — which returns reusable storage when `refs == 1`,
   having first dropped the node's own child references, and NULL otherwise.
   `nil_node_make` then consumes a token when the slot count matches. An
   unconsumed token must be freed at frame release, so tokens need slots too.

4. **Owned bindings inside a consuming arm only.** Taking the scrutinee drops
   its child references, so a borrowed binding would dangle. Bindings must be
   retained in arms that consume, and stay borrowed everywhere else, or the
   5.132× measured above regresses.
5. **Write only the changed slot.** Without this, reuse saves about two
   reference operations and two word writes per level, which is not enough to
   reach 3×.

Only (1)+(2)+(3) together make `refs == 1` observable inside the callee. With
(1) and (2) alone the count is still 2 at the construction site, so reuse never
fires and nothing is measurable; that is why this is not split further.

## What is still missing, as observed while writing v2

* **Deep recursion.** The binding constraint. A 256-frame native cap means the
  natural recursive solution is unusable on the frozen fixtures, so the token
  win cannot be collected. This outranks every other gap.
* **Prompt reclamation and unique-node reuse**, still owed from ADR 050 §3.
* **Cyclic structures.** Acyclicity is load-bearing for the memory design and
  simultaneously rules out doubly linked lists, so `lru-cache` stays blocked.
* **No closures or runtime-selected function values**, so a fold over a tree
  must be a named top-level function.
* **No strings, only `Bytes`**, and no formatting for records or sums, so
  structured output is hand-assembled byte by byte.
* **No generic user types**, so every container-like abstraction is monomorphic
  and hand-written per payload type.
* **No equality, ordering or hashing on sums**, so a sum cannot be a map key or
  be sorted without a hand-written projection to a scalar.
* **No wildcard arms, nested patterns or guards**, which keeps exhaustiveness
  exactly decidable but makes multi-level matches nest, as the BST validator's
  three stacked matches show.
