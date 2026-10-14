# Round 28 — nested-data ownership, integer-keyed maps and sets

## Unfavourable first

**The integer-keyed map was not built.** Item 3 and step B never started. The
raw-key path it needs is in place and the spelling study it required was not
run, so nothing about it is measured. It carries to Round 29 whole.

**Part 1 was never attempted.** The round opened by diagnosing the nested-data
copy and closed without fixing it. `--explain-copies` still has the blind spot
recorded below, which means **copy figures from earlier rounds are unreliable
too**, and the interprocedural child-edge transfer is untouched. Both move to
Round 29.

**Against Python the set still loses.** Bulk construction is the round's
largest token win and `contains-duplicate` is still **+71.4%** against
`len(set(a)) != len(a)`. The win was real and the comparison that matters still
goes the other way.

**Bulk construction is slower than the loop it replaces** — 1.829× C++ against
1.535× — because sizing for all-distinct keys over-allocates on a fixture full
of duplicates. Half the tokens cost a fifth of the speed.

**Only one of three promised rewrites landed.** `contains-duplicate` is the
only sampled natural program that uses a map at all, so the other two needed
genuine algorithmic rewrites, not a spelling change, and I did not write them.

**No problem newly qualifies.** `contains-duplicate` was already verified, so
the count stays **23/150 natural, 23/150 tuned**, unchanged for a fourth round.

**Three self-inflicted infra defects, all in my own code.** Two scheduler
defects from Round 27 (partial permit acquisition starving a full run to
10,481.7 s against 2,622.2 s of gate time; cargo gates sharing a target
directory with the depth audit) and then a defect in the Round 28 freeze that
was supposed to end that class: the unhashed `target/release/lib<crate>.rlib`
is not reliably refreshed, so it lagged five hours and the depth gate failed in
0.5 s. Fixed by freezing what cargo actually produced. The class is only dead
now that artifacts are resolved by mtime rather than by assumption.

**A known flake, not fixed.** `last-stone-weight` times out on the reference
path under heavy concurrency, reporting natural 1,069/1,070, and passes
**40/40 alone**. It needs 53.98 s of CPU against a 120 s budget, but the 960 s
wall backstop is reachable when the sanitizer runs beside it at 754–986 s.
Recorded rather than fixed, because infra was frozen for the round.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| set control vs `unordered_set` | 1.5–4× | **1.383×**, better than forecast |
| `contains-duplicate` kernel, incremental | 51 → 45, −11.8% | **45, −11.8%** — exact |
| `contains-duplicate` kernel, bulk | not forecast | **24, −52.9%** |
| integer hash needed? | likely | **no** — 1.383× left no case |
| two further problems rewritten | 5–15% each | **not attempted** |
| newly qualifying problems | 0–3 | **0** |
| integer-keyed map | shipped | **not started** |
| `course-schedule` v2 | 3–4× C++, honestly reported | **not measured** — Part 1 unstarted |

## natural-v2

Kernel-only cuts, cl100k and o200k agreeing exactly on every row.

| Program | v1 | v2 loop | **v3 bulk** | Python | v3 vs v1 | v3 vs Python | v3 / C++ |
|---|---:|---:|---:|---:|---:|---:|---:|
| contains-duplicate | 51 | 45 | **24** | 14 | **−52.9%** | **+71.4%** | **1.829×** |

Verified 40/40 by the harness, matching v1 on all eight frozen fixtures
(largest 639,037 bytes). The incremental form measured **1.535×** C++.

Spelling was chosen by measurement: `!set()`/`!add` cost 45 cl100k against 51
for `!format` keys on this kernel. The bulk kernel uses `!size` rather than
`#`, because `#` accepts only sequences and extending it to keyed collections
is a separate decision, not one to smuggle in here.

## What shipped

**The integer-keyed set, natively.** The map runtime's internal helpers now
take raw keys, `(const unsigned char *, int64_t)`, with the public
`NilSequence`-key entry points kept as thin wrappers so no existing caller
changed — proved by the 32 existing keyed tests still passing. The eight
big-endian key bytes are formed **inside the runtime**, so the emitter passes a
plain i64 and membership and insertion allocate nothing per call: no stack
sequence, no per-call allocation. Uniqueness comes from last use and is baked
into the function name, as for map updates. `!remove` preserves insertion
order, the iteration contract, by excising the entry and rehashing, which is
linear in entries and leaves membership and insertion untouched. `!set(v)`
builds from a sequence in one pass with first occurrence deciding order.

**No integer hash, because the measurement gave no case for one.** The control
inserts 200,000 keys at **1.383× `std::unordered_set`** over 15 interleaved
pairs, so the conditional A/B commit does not exist.

**The target-directory race class.** Gates read every artifact from a frozen
copy addressed through `NIL_BIN`, the two cargo gates have private target
directories, and `scripts/test_gate_isolation.py` runs a cargo build beside the
depth audit and the fuzz binary — the two readers that broke before — and
asserts all three succeed.

## Measured but not fixed

`course-schedule` **v1** is **193.882× C++** on the largest fixture (4,433.98
ms against 22.87 ms) while `--explain-copies` reports **zero copies**. A sample
profile put **98%** of samples in `_platform_memmove`, and instrumenting
`nil_set_unique_impl` gave exactly `32584 SET_COPY roots=2 len=20000`: every
update copies a 20,000-element buffer, about 5.2 GB, because the buffer arrives
with two roots where in-place needs one. The second root belongs to the caller,
which still holds the old record across the call, so a local borrow would
mutate a buffer the caller still points at — the transfer is **interprocedural**,
ADR 049 R3 territory. That 98% also bounds the available gain near 50×, so
193.9/50 ≈ 4× is the honest ceiling, and the 3× bar may simply be out of reach
on the last factor of two.

## Gates

Fast gates between steps; the closing full gate is the overseer's to run on a
quiet machine. The last fast receipt is `/private/tmp/r28-d2/receipt.json`:
corpus **2,815**, tuned **1,070/1,070**, depth **36/36**, sanitizer **309**,
fuzz seed 61001 **843 programs** with 0 non-resource divergences and 0 rule-3
failures, natural **1,069/1,070** from the flake above. Earlier full receipts:
`/private/tmp/r28-p2b/receipt.json` (1,400.9 s, all green) and
`/private/tmp/r28-g1/receipt.json` (1,628.8 s, all green, ci and ci-release
**646** each).

## Commits

```
c381cd1 docs(collections): preregister ADR 051 for nested ownership and integer keys
adfd0d0 docs(collections): quantify the nested-ownership copy and show it is interprocedural
409aa4e feat(collections): add an integer-keyed set, frontend and reference first
ace00e8 fix(gates): serialize admission and keep cargo gates off shared target
2b8b0d3 fix(gates): freeze read artifacts and isolate cargo target directories
e0e72c3 chore: advance the benchmarks pointer for NIL_BIN artifact resolution
6b1fcdd feat(collections): lower the integer set natively over the byte-keyed table
b6f7829 chore: advance the benchmarks pointer for the contains-duplicate rewrite
f7efea7 feat(collections): build a set from a sequence in one pass
04a7ed1 chore: advance the benchmarks pointer for bulk set construction
```

## Round 29

1. The integer-keyed map on the raw-key path, with the spelling study this
   round owed: `get`/`put`/`has`/`remove`/`size`/iterate in insertion order,
   and a control against `std::unordered_map`.
2. Part 1a: the `--explain-copies` blind spot at `roots == 2`, with a test
   asserting the site and reason, then a re-census of the natural cohort and an
   append-only note on every program whose earlier figures move.
3. Part 1b: interprocedural child-edge transfer, the only route to
   `course-schedule` v2 — expected to land near 4× C++, not under 3×.
4. The `last-stone-weight` wall backstop, now that infra can be touched again.
