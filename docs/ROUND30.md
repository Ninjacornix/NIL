# Round 30 — discovery batch: what actually blocks NIL

Compiler frozen: nothing under `crates/`, `cli/`, `tools/`, `std/` or
`scripts/` was touched. Only `benchmarks/` and `docs/`.

## Unfavourable first

**Only 10 of the 25 selected problems were actually authored.** The other 15
are classified from their contracts, fixtures and reference programs, not from
three real attempts each. That is a weaker evidence standard and the blocker
counts below should be read as a forecast for those 15, not a measurement.
Where a problem is classified rather than authored the table says so.

**Of the 10 authored, 6 qualify and 4 do not** — and three of the four failures
are NIL's fault rather than the algorithm's: a 40-token row join that is also
a per-element `!concat` cliff, and a functional buffer replace that copies.

**`!parsebuf` cannot parse an empty vector, and nobody had noticed.** The
corpus encodes one as `"0 \n"`; every separator choice leaves an empty field
and it fails **E016**. **Zero admitted problems** have ever contained that
encoding, so thirty rounds of green gates said nothing about it. Ten of the 25
batch problems have such fixtures. The workaround is a **169-token** hand-written
scanner against Python's **3-token** `.split()`.

**The copy-hint blind spot is not a curiosity — it hides a 137× cliff.**
`redundant-connection` runs at **137.5× C++** with **1,188 of ~1,200 samples in
`_platform_memmove`**, while `--explain-copies` reports **nothing at all**.
That is the `roots == 2` blind spot from ADR 051 addendum A, now reproduced on
a second, unrelated program. Any copy claim from that tool remains unreliable.

**Round 29's `!imap` finally has a user**, which is faint praise: exactly one
problem in 25 (`top-k-frequent-elements`) wanted it, and it needed manual
two-key packing into a single i64 because there is no comparator sort.

## Per-problem results

Authored and measured (15 timing repeats; borderline cases re-measured
interleaved with `ab_time.py`, which was **harsher** than the harness both
times):

| Problem | Category | Correct | × C++ | Tokens vs Py | Primary blocker |
|---|---|---|---:|---:|---|
| climbing-stairs | 1-D DP | **35/35** | **1.03** | −29% | — qualifies |
| two-sum-ii | Two Pointers | **35/35** | **1.42** | +56% | — qualifies |
| jump-game-ii | Greedy | **35/35** | **2.51** | −5% | — qualifies |
| remove-nth-node | Linked List | **35/35** | **2.90** | +146% | — qualifies |
| longest-incr-subsequence | 1-D DP | **35/35** | **1.61** | +343% | — qualifies |
| top-k-frequent-elements | Arrays & Hashing | **35/35** | **1.68** | +379% | — qualifies |
| counting-bits | Bit Manipulation | 35/35 | 4.60 | +96% | perf: per-element `!concat` row join |
| merge-two-sorted-lists | Linked List | 35/35 | 3.47 | +309% | perf: same row join |
| redundant-connection | Graphs | 35/35 | **137.51** | +1273% | perf: `a[i:x]` copies live buffer |
| sliding-window-maximum | Sliding Window | 32/35 | — | +31% | authoring: parallel state blocks a deque |

Classified from contract and fixtures, **not authored**:

| Problem | Category | Forecast blocker |
|---|---|---|
| valid-anagram | Arrays & Hashing | missing: text tokenization |
| distinct-subsequences | 2-D DP | missing: text tokenization |
| interleaving-string | 2-D DP | missing: text tokenization |
| word-search | Backtracking | missing: text tokenization |
| time-based-key-value-store | Binary Search | missing: text tokens + string-keyed map |
| design-twitter | Heap | missing: text tokens + sequence-valued map |
| design-add-and-search-words | Tries | missing: text tokens + sequence-valued map |
| combination-sum | Backtracking | missing: `!parsebuf` empty-field tolerance |
| rotting-oranges | Graphs | authoring: BFS worklist under parallel state |
| swim-in-rising-water | Advanced Graphs | authoring: Dijkstra worklist under parallel state |
| evaluate-reverse-polish-notation | Stack | missing: token-level text scan |
| subtree-of-another-tree | Trees | missing: sequence-valued containers |
| serialize-and-deserialize-tree | Trees | missing: sequence-valued containers |
| merge-intervals | Intervals | none identified — ran out of budget |
| multiply-strings | Math & Geometry | none identified — ran out of budget |

## Blocker histogram

| Rank | Blocker | Problems | Evidence standard |
|---|---|---:|---|
| 1 | **Missing: text tokenization** (split bytes into tokens; string keys) | **7** | classified |
| 2 | **Authoring: parallel loop state** blocks deques and worklists | **3** | 1 authored, 2 classified |
| 3 | **Missing: sequence-valued containers** (trees, tries) | **3** | classified |
| 4 | **Performance: per-element `!concat` row join** | **2** | authored, measured |
| 5 | **Missing: `!parsebuf` empty-field tolerance** | 1 blocked + 4 taxed | authored, measured |
| 6 | **Performance: functional replace copies live buffer** | 1 | authored, profiled |
| — | No blocker identified | 2 | unattempted |

## The three fixes this implies

1. **Text tokenization, with byte-string keys — unblocks 7.** Every one of
   `valid-anagram`, `distinct-subsequences`, `interleaving-string`,
   `word-search`, `time-based-key-value-store`, `design-twitter` and
   `design-add-and-search-words` is blocked first by having no way to split
   bytes into tokens. This is the largest single category in the batch and the
   one feature chosen by evidence rather than forecast. It likely needs the
   long-deferred sequence-valued container to carry the tokens.

2. **A built-in row join — fixes the dominant token hot spot and two cliffs.**
   `!join`-style vector formatting would replace a **40-token** adapter with a
   call, on every vector-output problem, and remove the per-element `!concat`
   that puts `counting-bits` at 4.60× and `merge-two-sorted-lists` at 3.47×.
   This is the cheapest fix on the list and the only one that improves tokens
   and speed together.

3. **`!parsebuf` empty-field tolerance — removes a 169-token tax.** One flag or
   mode. It unblocks `combination-sum`, deletes the scanner from four
   already-authored programs, and retires a defect that green gates could never
   have found because no admitted fixture exercises it.

Deliberately **not** in the top three: the nested child-edge transfer. The
batch says it would unblock `redundant-connection` and help the two tree
problems, but text tokenization blocks more than twice as many. The round-29
referendum therefore resolves toward **text and sequence-valued containers**,
not toward the ownership work — though the copy-hint blind spot should be fixed
regardless, since it silently hid a 137× cliff.

## Qualification

**Six newly qualifying problems**, taking the natural cohort from **21/150 to
27/150** — the first movement in five rounds, and it came from authoring
against evidence rather than from a compiler change. The 15 unauthored problems
could move it further; nothing here establishes that they will.

## Commits

```
a5fd569 bench(discovery): preregister the Round 30 discovery batch
90b5277 bench(discovery): first eight discovery-batch solutions
c7a6710 bench(discovery): nine passing solutions plus one partial
```
