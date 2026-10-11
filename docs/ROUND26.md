# Round 26 — data types checkpoint

## Unfavourable results first

**Native lowering of sums is not implemented.** Reference-counted nodes,
non-recursive drop and unique-node reuse are all unwritten. The LLVM entry
refuses any program containing a sum operation with **E009**, before emission,
so no native result is claimed and `nil run`/`build` reject these programs.
Consequently:

* **No speed measurement exists.** The ADR's forecast that the witness may miss
  3× C++ is neither confirmed nor refuted.
* **No sum fuzz families were added.** Differential fuzzing compares reference
  against native; with native refused there is nothing to compare. The ADR
  requires families for every variant shape, deep lists, aliasing and node
  reuse — all of those depend on native lowering and remain owed.
* **The 10⁶-node non-recursive drop requirement is untested.** It cannot be
  exercised on the reference path at all: building a list by source recursion
  hits the call-depth guard first, so `E008` arrives long before any drop. A
  500-node list builds and folds to its triangular number, and the same program
  under a small explicit depth limit reports E008 rather than crashing; both are
  pinned by `deep_list_depth_is_bounded_by_the_call_guard_not_by_the_drop`.
  Reaching 10⁶ nodes needs an iterative builder under native execution, so this
  obligation is deferred with the native work, not satisfied.

**B2 (Option), B3 (nested containers) and cohort natural-v2 are not started.**
Qualification therefore still stands at natural 21/150, tuned 23/150, with 7
validation-blocked and 114 unattempted problems. Nothing in this round has yet
moved the headline product metric.

**The headline token result is a source measurement, not a verified program.**
The 107-token v2 kernel below typechecks (`nil check` returns `ok`) and
equivalent programs execute correctly under reference execution in
`crates/nil-compiler/tests/sums.rs`, but it has not been run against the frozen
fixtures, because that needs native execution.

## What the round established

[ADR 050](adr/050.md) was committed before any code and settles the design.
The measurement that motivates it, on the frozen kernel cut of
`balanced-binary-tree` (tiktoken 0.12.0, cl100k):

| Source | cl100k | vs Python |
|---|---:|---:|
| NIL v1 — index links, explicit stack, memo buffer | 292 | **+78.0%** |
| Python kernel | 164 | — |
| C++ kernel | 203 | +23.8% |
| **NIL v2 — sum + recursive + match** | **107** | **−34.8%** |

A **63.4% token reduction**, flipping this program from 78% worse than Python to
35% better. The deficit on algorithmic code was the integer-link and
offset-arithmetic workaround, not the surface syntax — which is what ADR 024
could not determine from attribution evidence alone.

The v2 kernel in full:

```nil
sum T(N(i,T,T),L)
(v)=c(b(a,a[1]))>=0?1:0
(v,i):T=b==-1?L:N(a[2+3*b],b(a,a[3+3*b]),b(a,a[4+3*b]))
(T)=?a(N(_,l,r):(x=c(l);y=c(r);x<0?-1:y<0?-1:!abs(x-y)>1?-1:1+!max(x,y)),L:0)
```

### Spelling by measurement

Both tokenizers agreed within one token on every candidate, so only cl100k is
tabulated. Declaration candidates were within one token of each other, so the
tie broke on consistency with `record Name(...)` and on freedom from ambiguity.

| Declaration | cl100k | |
|---|---:|---|
| `sum Tree(Node(i,Tree,Tree),Leaf)` | 11 | **chosen** |
| `enum …` / `type …` / `sum Tree=…\|Leaf` / `record Tree(…\|Leaf)` | 11 | |
| `Tree=Node(i,Tree,Tree)\|Leaf` | 10 | rejected: `Name=` is ambiguous with a positional function |

| Match (measured over a complete four-function program) | cl100k | |
|---|---:|---|
| `?a(N(_,l,r):X,L:Y)` | 156 | **chosen** |
| `a?(N(_,l,r):X,L:Y)` | 156 | rejected: collides with the ternary `a?(x):(y)` |
| `match a(…)` / `?a N(…):X;L:Y` / `?a(N(…)=>X,L=>Y)` | 157 | |
| `?a{N(…):X,L:Y}` | 158 | rejected: `{}` is record update |

| Option | cl100k | |
|---|---:|---|
| `?!get(a,b)(?v:v,?:0)` — `?T` sugar | 17 | rejected: overloads `?` a third time |
| `?!get(a,b)(Some(v):v,None:0)` — built-in monomorphized | 18 | **chosen** |
| user-declared `sum OptI(Some(i),None)` + match | 26 | free anyway under B1 |

| Nested containers (`course-schedule` adjacency build) | cl100k |
|---|---:|
| v1 `record Adj(head:v,next:v,to:v)` index links | 88 |
| `v[v]` sequence of sequences | **64** |
| `map[v]` keyed adjacency | 81 |

**A grammar ambiguity the token measurement did not catch:** `?a(…)` collides
with a call, since `a(` is a direct call. Resolved by making a `(` end the match
head, so the scrutinee is a primary without a call suffix; a call scrutinee must
be parenthesized. Costs nothing in the common case, where the scrutinee is a
parameter or local.

### What landed

**B1 frontend and reference semantics.** Declarations now form one group, so a
sum may refer to itself or forward to a later one. Records stay acyclic and
earlier-only: they are inline aggregates with no indirection, so recursion goes
through a sum payload, which is one opaque slot. Sums are checked for
inhabitation, so `sum X(OnlyX(X))` is rejected rather than failing confusingly
downstream.

`match` desugars to a tag read plus nested lazy `If` regions, so arm laziness,
rooting and liveness reuse existing machinery and HIR gains no control form.
Two new exact Check diagnostics: **E025** non-exhaustive match, naming the
uncovered variants, and **E026** unreachable arm. Codes through E024 were
already allocated.

**One soundness hole found and closed during implementation.** Variant names
resolve across the whole declaration group, so a match whose arms belong to
another sum would have silently selected the wrong variant with no diagnostic
whenever the arms' payload types happened to agree — and always when every arm
is nullary. `SumTag` and `SumField` now carry their expected sum type and the
validator requires the operand to match, which turns that case into an exact
E007. Covered by `matching_a_scrutinee_from_another_sum_is_rejected_exactly`.

**B4 complete, and it was a specification defect, not a code defect.** Record
buffers already reported E014 for an invalid length, matching flat buffers;
`EXPR_V5.md` documented E013 and never matched the implementation. The document
is corrected and the behaviour is pinned on the reference path and natively at
both O0 and O2.

## Status by step

| Step | State |
|---|---|
| ADR 050 | committed before code (`c945eaa`, verified by `git log --reverse`) |
| B1 sums + recursive + match | frontend, validation and reference execution land; **native lowering refused with E009**, so incomplete |
| B2 Option | not started |
| B3 nested containers | not started |
| B4 record-buffer E014 | complete: specification corrected, behaviour pinned at reference/O0/O2 |
| natural-v2 cohort | not started |

## Gates

Run at `0eb1424`.

| Gate | Result | Required |
|---|---|---|
| `./scripts/ci.sh` | **622 passed**, 0 warnings | ≥596 |
| `./scripts/ci.sh release` | **622 passed**, 0 warnings | ≥596 |
| `./scripts/sanitize.sh` | **274 passed**, no findings, no leaks | ≥274 |
| `fuzz-v5.sh --seed 61001` | 821 programs, 1,754 native builds, **0 divergences, 0 non-resource divergences, rule_3_failures 0**, 36 E008 inconclusive | 0 / 0 |
| `fuzz-v5.sh --seed 83003` | 821 programs, 1,754 native builds, **0 divergences, 0 non-resource divergences, rule_3_failures 0**, 36 E008 inconclusive | 0 / 0 |
| application corpus | 2,815 checks, 0 inconclusive | 2,815 |
| application corpus `--group original` | **exactly 610** | 610 |
| NeetCode `--verified-only` | 1,070 / 1,070 | 1,070 |
| NeetCode natural cohort | 1,070 / 1,070 | 1,070 |
| `audit_depth.py` | **36/36** nondivergent, 0 failed | 36/36 |

The tests over the 596 baseline are 8 sum unit tests in `nil-hir` and 18
integration tests in `crates/nil-compiler/tests/sums.rs`. Every frozen corpus
count is unchanged, which is the proof that the new syntax altered no existing
program. No frozen input was edited.

## Commits

```
c945eaa docs(types): preregister sum, recursive, match, Option and nested containers
a04291c feat(types): add sum types, recursive declarations and pattern matching
0eb1424 fix(docs): record buffers report E014 for an invalid length, not E013
```

Branches `feat/core-types` (from `origin/master` 7af71c6) and submodule
`exp/core-types` (from 098f8f4). Nothing pushed; nothing amended.

## What is still missing, as observed while writing v2

* **Native sum lowering** — the blocker for every remaining deliverable.
* **An iterative builder and a worklist drop**, without which the 10⁶-node
  obligation cannot even be expressed: source recursion exhausts the call guard
  first.
* **No closures or runtime-selected function values.** Higher-order code stays
  statically specialized, so a fold over a tree must be a named function.
* **No strings, only `Bytes`.** Byte indexing is not character indexing, and
  there is no formatting for records or sums, so any structured output is
  hand-assembled.
* **No generic user types.** `Option` has to be built in because a user sum
  cannot be parameterized, so every other container-like abstraction must be
  monomorphic and hand-written per payload type.
* **No comparison, equality or ordering on sums**, so a sum cannot be a map key
  or sorted without a hand-written projection to a scalar.
* **Wildcard arms, nested patterns and guards are absent**, which keeps
  exhaustiveness exactly decidable but makes multi-level matches nest.
