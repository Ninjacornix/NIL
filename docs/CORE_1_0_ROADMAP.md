# Core 1.0 qualification roadmap

**Core 1.0 is not complete: 22/150 meet the first frozen suite’s correctness and ≤3× C++ gates (19/150 at Round 19). Seven of the 36 attempted programs fail validation limits; seven pass correctness but miss performance; 114 have no NIL attempt.** Counts describe implementations, not 128 missing language features.

**Round 20 status (ADR 042).** CORE-201's source-only read-before-write scheduling was implemented as immutable local bindings. Three of its five targets now qualify: `balanced-binary-tree` 1.99×, `validate-binary-search-tree` 1.84×, `course-schedule` 1.83×. `spiral-matrix` improved from 170.98× to 3.30× and `last-stone-weight` from 2,299.45× to 168.89×; both still miss, so CORE-202's field-sensitive alias/liveness and callee-loop ownership work remains open for those two. Scalar operators, constants and bindings now exist, which removes the modulo/bitwise/min-max emulation items from C5 as *written* but not as *measured* unlocks. The C3 serialization/builder group and `longest-repeating-character-replacement` are unchanged. [Round 20 report](../benchmarks/reports/2026-10-07/ROUND20.md).

[ADR 041](adr/041.md) defines Core 1.0, STL 1.0 and post-1.0 scope. The [measured report](../benchmarks/reports/2026-10-07/CORE_1_0.md), [150-task preregistration](../benchmarks/corpora/neetcode150/CENSUS.md) and [status ledger](../benchmarks/corpora/neetcode150/STATUS.json) are the yardstick. Public names/categories are the only imported metadata. All contracts, fixtures, oracles and solutions are original.

## What the census actually establishes

The preregistered forecast was 150 semantically feasible and 98 performance qualifiers. In the fixed sample, 22 expected qualifiers became 19: four unexpected successes, seven missed forecasts. Reference deadlines and E008 failures make seven candidates operationally blocked; no absent core capability has yet been proved necessary. Index pools, flat grids, nested record buffers, byte-key maps and explicit stacks can express the algorithms. Modulo/bitwise emulation and conditional min/max/abs can be written today; convenience is not a Core 1.0 prerequisite.

Published feature tags are risk flags, not complete dependency graphs. Many ordinary string tasks were not assigned the narrower ASCII-classification risk tag. The separate [feature audit](../benchmarks/corpora/neetcode150/FEATURE_AUDIT.json) records 34 algorithmic byte/text tasks, preserving the preregistration. Inspect each original byte contract and reference algorithm before using the tag counts to justify a feature. No quota-fit or deep-recursion claim is proven for the unattempted 114.

## Ordered qualification rounds

Effort estimates are engineering days for one contributor, not calendar promises. Each task can be executed independently after its dependencies. Every round preserves frozen task contracts, oracles, goldens and baseline sources; failed attempts remain explicit. Compiler changes require separate pre-registered ADRs, reference/O0/O2 parity, sanitizers and differential fuzzing that compares effects.

### C1 — Repair the seven failed candidates (8–15 days)

**CORE-101:** indexed doubly linked list plus lookup for `lru-cache`; remove the current O(operations × keys) timestamp scans. **CORE-102:** a handwritten minimum heap for `course-schedule-ii` and heap Dijkstra for `network-delay-time`, matching the C++ algorithm classes. These need no new type or heap intrinsic. **CORE-103:** indexed trie pool for `implement-trie-prefix-tree` and shared-trie DFS for `word-search-ii`, replacing prefix-map/per-word work. **CORE-104:** isolated reference/native validation of `n-queens` and `cheapest-flights-within-k-stops`, attributing evaluator cost before considering compiler work.

Dependencies: suite v1 and unchanged limits. Intended unlocks: all seven presently unadmitted samples. Tests: every frozen fixture including largest, exact output/effects at reference/O0/O2, then ≥15 serial timing repetitions. Acceptance: admitted source only after every execution gate; parity must use the same algorithm class. Risks: copying inside callees and interpreter latency may remain. Source-only algorithm changes do not alter arena/root proofs.

### C2 — Remove measured update cliffs (10–20 days, proof work conditional)

**CORE-201:** controlled source-only read-before-write scheduling trials on `last-stone-weight`, `balanced-binary-tree`, `validate-binary-search-tree`, `course-schedule` and `spiral-matrix`. Capture old scalar reads before functional updates without changing observable order. Measure allocations/copy bytes and ownership decisions; guarded `nil_set_unique` IR counts alone are insufficient. These five are correct but 108×–2,299× slower in the current sample.

**CORE-202:** only if scheduling cannot exercise existing proofs, pre-register field-sensitive alias/liveness and callee-loop unique ownership changes. Preserve every live alias, return alias, lazy arm, root descendant, left-to-right trap and quota observation. Stop on unexplained divergence. No new user-visible ownership syntax or weakened quota.

Dependencies: CORE-201 attribution before CORE-202. Intended unlocks: those five performance misses and improved C1 heap/trie implementations. Acceptance: same frozen outputs and ≤3× median with variance published. This round may change load-bearing arena/root/borrow proofs; it is the highest-risk core work. It cannot be replaced by simply raising 64 MiB or the step limit.

### C3 — Serialization and builder qualification (5–10 days)

**CORE-301:** measure serialization versus kernel cost for `3sum`, `generate-parentheses`, `letter-combinations-of-a-phone-number`, `plus-one`; inspect temporary format/slice/concat chains and recursive builders. **CORE-302:** isolate byte-frequency/update cost for `longest-repeating-character-replacement`. Fix authoring/library algorithm composition first; invoke C2 proof work only for a measured compiler boundary.

Intended unlocks: the other five measured performance misses. Acceptance: no changed outputs/order, no problem-specific intrinsic, same 3× bound and original max sizes. Algorithm/codecs belong in STL unless a primitive or ownership proof is demonstrated necessary. Risks: output volume and shared recursive builders may remain conservative; publish curves instead of claiming a missing serializer alone is a core gap.

### C4 — Cover the remaining 114 (20–40 days, category batches)

**CORE-401..418:** implement the unattempted tasks below, one category per independent work item. Use existing types and efficient reference-matching algorithms first. Record missing capabilities, E008/depth/quota deadlines, correctness and performance separately. Neither a hash-sample forecast nor successful tiny cases count as qualification.

Dependencies: suite v1; batches can run alongside C1 source work. Each deliverable: admitted sources, full fixture receipt, ≥15 largest-case timing repeats, source counts and an updated status ledger. Acceptance: every task attempted, no task silently removed. Proof changes discovered here become separate scoped rounds, not ad hoc features.

- **CORE-401 — Arrays & Hashing (7):** `valid-anagram`, `two-sum`, `group-anagrams`, `top-k-frequent-elements`, `product-of-array-except-self`, `encode-and-decode-strings`, `longest-consecutive-sequence`.
- **CORE-402 — Two Pointers (3):** `two-sum-ii-input-array-is-sorted`, `container-with-most-water`, `trapping-rain-water`.
- **CORE-403 — Sliding Window (4):** `best-time-to-buy-and-sell-stock`, `permutation-in-string`, `minimum-window-substring`, `sliding-window-maximum`.
- **CORE-404 — Stack (5):** `valid-parentheses`, `min-stack`, `evaluate-reverse-polish-notation`, `daily-temperatures`, `largest-rectangle-in-histogram`.
- **CORE-405 — Binary Search (5):** `binary-search`, `search-a-2d-matrix`, `koko-eating-bananas`, `find-minimum-in-rotated-sorted-array`, `time-based-key-value-store`.
- **CORE-406 — Linked List (9):** `merge-two-sorted-lists`, `reorder-list`, `remove-nth-node-from-end-of-list`, `copy-list-with-random-pointer`, `add-two-numbers`, `linked-list-cycle`, `find-the-duplicate-number`, `merge-k-sorted-lists`, `reverse-nodes-in-k-group`.
- **CORE-407 — Trees (13):** `invert-binary-tree`, `maximum-depth-of-binary-tree`, `diameter-of-binary-tree`, `same-tree`, `subtree-of-another-tree`, `lowest-common-ancestor-of-a-binary-search-tree`, `binary-tree-level-order-traversal`, `binary-tree-right-side-view`, `count-good-nodes-in-binary-tree`, `kth-smallest-element-in-a-bst`, `construct-binary-tree-from-preorder-and-inorder-traversal`, `binary-tree-maximum-path-sum`, `serialize-and-deserialize-binary-tree`.
- **CORE-408 — Tries (1):** `design-add-and-search-words-data-structure`.
- **CORE-409 — Heap / Priority Queue (5):** `kth-largest-element-in-a-stream`, `k-closest-points-to-origin`, `task-scheduler`, `design-twitter`, `find-median-from-data-stream`.
- **CORE-410 — Backtracking (7):** `subsets`, `combination-sum`, `permutations`, `subsets-ii`, `combination-sum-ii`, `word-search`, `palindrome-partitioning`.
- **CORE-411 — Graphs (11):** `number-of-islands`, `clone-graph`, `max-area-of-island`, `pacific-atlantic-water-flow`, `surrounded-regions`, `rotting-oranges`, `walls-and-gates`, `redundant-connection`, `number-of-connected-components-in-an-undirected-graph`, `graph-valid-tree`, `word-ladder`.
- **CORE-412 — Advanced Graphs (4):** `reconstruct-itinerary`, `min-cost-to-connect-all-points`, `swim-in-rising-water`, `alien-dictionary`.
- **CORE-413 — 1-D Dynamic Programming (10):** `climbing-stairs`, `house-robber`, `house-robber-ii`, `longest-palindromic-substring`, `palindromic-substrings`, `decode-ways`, `maximum-product-subarray`, `word-break`, `longest-increasing-subsequence`, `partition-equal-subset-sum`.
- **CORE-414 — 2-D Dynamic Programming (9):** `unique-paths`, `longest-common-subsequence`, `coin-change-ii`, `interleaving-string`, `longest-increasing-path-in-a-matrix`, `distinct-subsequences`, `edit-distance`, `burst-balloons`, `regular-expression-matching`.
- **CORE-415 — Greedy (6):** `jump-game`, `jump-game-ii`, `hand-of-straights`, `merge-triplets-to-form-target-triplet`, `partition-labels`, `valid-parenthesis-string`.
- **CORE-416 — Intervals (4):** `merge-intervals`, `non-overlapping-intervals`, `meeting-rooms-ii`, `minimum-interval-to-include-each-query`.
- **CORE-417 — Math & Geometry (6):** `rotate-image`, `set-matrix-zeroes`, `happy-number`, `powx-n`, `multiply-strings`, `detect-squares`.
- **CORE-418 — Bit Manipulation (5):** `number-of-1-bits`, `counting-bits`, `reverse-bits`, `missing-number`, `sum-of-two-integers`.

### C5 — Conditional primitive rounds, justified only by C4 measurements (2–8 days each)

No direct primitive below is yet proved mandatory. Lists identify **candidate beneficiaries**, not guaranteed unlocks. Re-register forecasts from actual failing programs before implementation; retain conservative checked behaviour.

- **Modulo (6 risk-tagged tasks):** `koko-eating-bananas`, `add-two-numbers`, `happy-number`, `plus-one`, `multiply-strings`, `reverse-integer`. Signed zero-divisor and MIN/-1 ordering must agree with division. No arena change.
- **Bitwise and shifts (6 risk-tagged tasks):** `n-queens`, `single-number`, `number-of-1-bits`, `counting-bits`, `reverse-bits`, `sum-of-two-integers`. Exact width/sign/shift-range semantics and reference/native parity; no arena change.
- **Integer-keyed maps/sets (9 risk-tagged tasks):** `contains-duplicate`, `two-sum`, `top-k-frequent-elements`, `longest-consecutive-sequence`, `copy-list-with-random-pointer`, `lru-cache`, `construct-binary-tree-from-preorder-and-inorder-traversal`, `hand-of-straights`, `detect-squares`. Existing decimal byte keys work; implement only if measured hash/key allocation cost blocks qualification. Layout, capacity, reuse and root proofs change.
- **Grid/nested collection access (32 risk-tagged tasks):** `group-anagrams`, `time-based-key-value-store`, `implement-trie-prefix-tree`, `design-add-and-search-words-data-structure`, `word-search-ii`, `design-twitter`, `number-of-islands`, `clone-graph`, `max-area-of-island`, `pacific-atlantic-water-flow`, `surrounded-regions`, `rotting-oranges`, `walls-and-gates`, `word-ladder`, `reconstruct-itinerary`, `min-cost-to-connect-all-points`, `swim-in-rising-water`, `cheapest-flights-within-k-stops`, `word-break`, `unique-paths`, `longest-common-subsequence`, `target-sum`, `interleaving-string`, `longest-increasing-path-in-a-matrix`, `distinct-subsequences`, `edit-distance`, `burst-balloons`, `regular-expression-matching`, `rotate-image`, `spiral-matrix`, `set-matrix-zeroes`, `detect-squares`. Existing flat/indexed/nested storage is available; a new type is not justified by this census. Any new layout needs full quota/alias/root validation.
- **Heap/priority queue (9 risk-tagged tasks):** `merge-k-sorted-lists`, `kth-largest-element-in-a-stream`, `last-stone-weight`, `design-twitter`, `find-median-from-data-stream`, `course-schedule-ii`, `network-delay-time`, `swim-in-rising-water`, `minimum-interval-to-include-each-query`. Writable with current buffers; algorithm/library first. New core surface only if a primitive-proof obstacle remains.
- **Deep recursion (11 risk-tagged tasks):** `maximum-depth-of-binary-tree`, `diameter-of-binary-tree`, `balanced-binary-tree`, `same-tree`, `subtree-of-another-tree`, `count-good-nodes-in-binary-tree`, `validate-binary-search-tree`, `kth-smallest-element-in-a-bst`, `binary-tree-maximum-path-sum`, `serialize-and-deserialize-binary-tree`, `reconstruct-itinerary`. Explicit stacks are available; changing 256 frames needs evidence they cannot satisfy the contract. Root-stack/call-limit policy is load-bearing.
- **Floating math (1 risk-tagged tasks):** `powx-n`. Current binary64 and exponentiation by squaring may suffice; retain bit-exact no-FMA semantics.

Logical operations, early exit and abs/min/max already compose through lazy conditions and state. No new feature round is planned without a failing contract or measured bottleneck. Unicode, concurrency, FFI, general regex, views and runtime closures remain post-1.0. Problem-specific operations remain forbidden.

### C6 — Release-grade suite audit and spec freeze (5–10 days)

**CORE-601:** audit all contracts for multidimensional bounds and worst-case input/output volume, and cross-check original oracles independently. Suite v1 largest fixtures are not exhaustive: a random duplicate input may exit lookup early; sparse graphs do not prove dense maximum behaviour. Expand adversarial coverage only as a versioned, pre-registered suite revision, never alter frozen goldens quietly. **CORE-602:** run all 150 reference/O0/O2 fixtures and largest-case 3× comparisons on the audited release suite, retaining failures and uncertainty. **CORE-603:** consolidate a normative versioned Core 1.0 spec and migration notes after syntax/semantics freeze.

Dependencies: C1–C4 coverage, any demonstrated C5 work, then CORE-601 before final qualification. Acceptance: all 150 fully qualify with diagnostic/effect parity, compiler gates and proof validation clean. Default expr-v0 stays until an explicit maintainer decision following this evidence. No automatic profile switch.

## Round 19 receipts (suite v1 baseline)

No compiler/runtime/profile or workspace dependency changes. CI debug/release: 466 tests each, zero failures/Clippy warnings. New suite: 150 Python/C++ tasks, 2,126 baseline checks; 29 admitted NIL programs, 1,070 checks across all five paths; 12 harness tests. Existing application corpus remains 2,815 checks and original group exactly 610. The report links raw artifact hashes and commands. Source density on the 29 survivors is ~19% below Python, but is not representative all-150 density or model token efficiency. Round 20 later showed that figure to be driver-sensitive: the same programs cost 66.7% more cl100k tokens than Python once file adapters are excluded in every language.

Core 1.0 measures executable coverage and bounded performance. STL 1.0 then measures concise reusable solutions and failure-inclusive model tokens, with source density as a separate metric. The unfavourable generation result (NIL 0/24, Python 11/24) remains unchanged.
