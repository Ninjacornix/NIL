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

Every slot change goes through `nil_root_store`. The header counts active roots;
`nil_live` charges an allocation exactly while its root count is nonzero. Collection
before actual allocation/read frees zero-root arena entries. Unique replacements
check `nil_live` and the cached root count without a full arena/root scan. Zero-root
values survive transfer gaps until the next collection; immediate freeing on the
last root removal would be unsound. Root-frame removal clears its remaining slots.
The driver uses the same store helper for argument admission. Sequences contain
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

## Root retention, length hoisting and LTO

A separate root-retention proof covers only straight identity/replacement state
chains, last-use replacement operands, and scalar-only conditions/bodies. Calls,
other allocations and nested/lazy regions force the original transfer protocol.
The state slot is initialized in the preheader, reused for body input and each
replacement result, retained at yield, and cleared before finish-region transfer.
A copy updates that same slot to the new pointer; runtime alias counting still
forces copying when another state slot/caller retains the original. Scalar-only
conditions cannot collect, allocate or replace, so their duplicate input roots
are unnecessary. This does not narrow the existing replacement proof: lazy
replacement chains still reuse under the conservative root protocol.

Known-dead slots are removed from the emitter's root list after clearing, preventing
repeated null stores at the same death boundary or region exit. Unproved roots stay.
Identity state and proved replacement chains preserve sequence length; `nil_length`
is nontrapping and loaded in the loop preheader. Length-changing state is not hoisted.

V5 O2 compiles emitted IR and the C runtime as LTO bitcode. Checked accessors and
root updates are explicitly always-inline; merely enabling LTO left calls in the
measured binary. Their C implementations remain the single source of truth for
access checks, quota, alias handling and stores. No duplicate layout/bounds lowering
is hand-emitted. O0 retains separate compilation; earlier profiles are unchanged.
Linux O2 application builds require an LTO-capable LLD linker; macOS uses Apple ld.

## Bulk file reads

`fstat` supplies a capacity hint for regular files, never a quota/failure oracle.
`fread` fills an unpublished sequence payload; streams grow in bounded chunks.
The available semantic budget is computed once. At most one byte beyond that
budget is read, so an oversized successful prefix still fails E013 before a later
I/O failure. Otherwise read/close failures produce E015 before final admission,
including the empty-read/header-quota case. Truncation/growth does not turn a stale
size hint into E013 or silently truncate a file. The payload is linked into the
arena only after successful I/O/admission, avoiding a second full-file copy.
Pipes still require EOF; there is no I/O deadline or recoverable failure guarantee.

## Verification and limits

Tests cover both selected IR paths, live aliases, exact read admission, streamed
binary input, invariant and changing lengths, root retention's negative obligations,
and complete 1 MiB transforms at O0/O2 including double-transform round trips.
Three seeded campaigns exercise old reads, retained caller arguments, post-loop
captures, lazy arms and returned aliases. ASan/UBSan instruments emitted code and
C runtime; leak checking remains disabled. These tests do not prove arbitrary
compiler correctness. See [runtime command evidence](../validation/APPLICATION_RUNTIME.md)
and the [before/after file benchmark](../../benchmarks/reports/2026-10-02/APPLICATION_RUNTIME.md).
