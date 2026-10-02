# Dynamic storage invariants

## Semantic budget

V5 changes from cumulative allocation charges to live allocation charges. The
limit remains 64 MiB. A unique dynamic payload costs bytes + 32. Multiple roots
to one allocation cost once. At an allocating instruction, operands and its
transient result charge count together; reuse reserves this same charge before
mutation. This makes E013 independent of whether replacement physically copies.
Dead results/operands no longer accumulate across a loop. Bookkeeping and host
scratch space are outside the language budget, so this is not a process RSS cap.

## SSA roots and reclamation

HIR last-use analysis returns a linear-size table. Branch captures are conservatively
unioned; loop regions use fresh IDs and capture only initial values. Region results
remain live to yield. The evaluator prunes dead frame positions while preserving
SSA numbering, counts distinct Arc allocations across frames and condition state,
and uses Arc COW for dead replacement operands. External Rust argument handles
are not execution roots; COW still protects their immutable contents.

The native compiler allocates a zero-initialized root-slot array per function.
Parameters, region-local values and loop state enter/leave slots at their semantic
boundaries. Calls/branches/loops transfer operands before any subsequent allocation;
no arena allocation or collection occurs between removing a parent root and registering child roots,
or between yielding a region result and rooting it in the parent. Root-registry
bookkeeping may allocate host memory in this gap but cannot collect the arena. Suspended callers
retain their future-live values. Loop header roots protect condition/exit state and
transfer to body/finish inputs before allocation. Root arrays leave the registry on
function return. The driver roots partially constructed arguments during admission.

Before allocation (and before buffered file-read quota checks), collection marks
all active slot pointers and frees unreachable arena entries. Sequences contain
only flat scalar payloads, so there are no internal graph edges. The C header keeps
byte data eight-byte aligned for Buffer access. All allocations are reclaimed at
execution exit after the result is consumed.

## Reuse proof

The fixed-array analysis proves a single-use result chain and defers writes into
private state storage until the backedge. That original proof intentionally allows
later reads of old state. Applying it unchanged to dynamic immediate writes would
be unsound. Dynamic lowering adds two explicit obligations:

1. The replacement is in a proved loop-carried chain (including selected lazy arms)
   and the old SSA operand has no use after this instruction. Bounds/value expressions
   finish evaluation before replacement; intermediate results have no escaping uses.
2. Runtime root counting finds exactly one active slot pointing at the allocation.
   Another caller binding, duplicated loop state or captured alias forces copying,
   even when its identity differs from the current SSA ID.

`nil_set_unique` checks bounds, byte range and the transient budget in that order.
Only then may it mutate. A live alias calls ordinary copying `nil_set`. A static
old read prevents emitting `nil_set_unique` at all. Fixed-array lowering remains
unchanged. No user-visible mutable values, views, address identity or ownership
syntax are introduced. Branches preserve lazy effects, and failed operations cannot
commit a write before their diagnostic checks.

## Verification and limits

Tests assert both selected IR paths, preserve original reads, exercise live aliases,
verify a complete 1 MiB file transform at O0/O2 and compare reference/native results.
Two seeded campaigns include old reads, retained caller arguments, post-loop captures,
lazy identity arms and returned values with live aliases. A separate regression verifies that aliased
reference-entry arguments are charged once during admission. ASan/UBSan instruments
both emitted code and C runtime. These checks do not prove arbitrary compiler
correctness or absence of leaks; leak checking remains disabled. Root scans and
opaque runtime calls remain expensive, and the measured file loop is not at C++
speed. See [command evidence](../validation/DYNAMIC_STORAGE.md) and the
[file benchmark](../../benchmarks/reports/2026-10-02/APPLICATION_STORAGE.md).
