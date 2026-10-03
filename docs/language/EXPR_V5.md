# expr-v5 — application data and host operations

Experimental, opt-in with `--profile expr-v5`. Earlier profiles and the default
remain unchanged. This extends v4's wrapping arithmetic, lazy branches, ordered
argument evaluation and immutable values. The [baseline application density study](../../benchmarks/reports/2026-10-03/V5_DENSITY.md) measured 36.7–43.5% more source tokens than Python before the generality additions below. The
[first generation/repair study](../../benchmarks/reports/2026-10-03/V5_GENERATION.md)
was unfavourable: v5 solved 0/24 application trials against Python's 11/24 and
spent more total input/output tokens under the two fixed local models. This
measurement does not change language semantics or the default profile.

## Types and inputs

In typed signatures, `v` means a runtime-sized sequence of i64 values; `s` means a
runtime-sized byte sequence. `i`, `b`, and numeric fixed-array lengths retain v4
meanings. Text literals encode Unicode as UTF-8 bytes. Indexing operates on bytes,
not Unicode characters. Bytes need not contain valid UTF-8.

```text
(v)=#a
(s):s=!concat("Hello, ",a)
(s,s)=!write(b,!read(a))
```

A native entry takes one argument per `s` or `v` parameter: text for `s`, a compact
comma-separated integer list such as `[1,-2,3]` for `v`. Fixed arrays still consume
flattened decimal slots. Returned buffers print integer lists; byte results write
raw bytes, followed by the CLI's result newline. These are private tooling
conventions, not a public pointer or C ABI.

## Operations

`#a`, `a[i]`, and `a[i:value]` support dynamic sequences as well as fixed arrays.
Replacement returns a new equal-length value; aliases remain unchanged. Byte
replacement values must be 0..255. Quoted literals support `\n`, `\r`, `\t`, `\0`,
`\"`, `\\`, and `\xNN` (exactly two hexadecimal digits). A hex escape inserts
one byte, including non-UTF-8 bytes: `"\xFF\x00"` contains bytes 255 and 0.
Literal newlines must be escaped.

| Operation | Inputs | Result |
|---|---|---|
| `!buffer(length,fill)` | i64, i64 | v |
| `!bytes(length,fill)` | i64, i64 (fill 0..255) | s |
| `!concat(a,b)` | matching v/v or s/s | same sequence type |
| `!slice(a,start,length)` | v or s, i64, i64 | same sequence type |
| `!format(value)` | i64 | decimal bytes |
| `!parse(text)` | s | i64 |
| `!parsebuf(text,separators)` | s, s | v |
| `!equal(a,b)` | matching v/v or s/s | bool |
| `!find(sequence,element,start)` | v or s, i64, i64 | i64 |
| `!read(path)` | s | file bytes |
| `!write(path,data)` | s, s | bytes written as i64 |
| `!out(data)` | s | bytes written to stdout as i64 |

Slices return newly allocated copies, including when the full input is selected;
they are not borrowed views. Returning a parameter preserves its immutable value.

Lengths must be nonnegative; slice start/length must fit the sequence, including
empty slices at its end. Decimal parsing accepts only canonical i64 strings:
no whitespace, leading plus, leading zeroes, negative zero, or trailing newline.
File paths are OS bytes and cannot contain NUL. Writes create/truncate their target;
failures may leave partial output. No atomic-write guarantee is provided.

## Bulk parsing, equality and delimiter iteration

These are typed semantic HIR instructions under the existing intrinsic grammar,
not a new profile or parser extension. Earlier profiles/default are unchanged.

`equal` compares lengths and elements; matching empty sequences compare equal.
Buffer comparison compares signed i64 values, byte comparison compares raw bytes.
It does not compare allocation identity, convert types or decode text. It allocates
nothing, has no host effect and introduces no runtime failure for valid operands.
Mismatched sequence types/arity/results are rejected by HIR checking (E007/E006).

`find` returns the first matching index at or after `start`, or sequence length
when absent. Start equal to length is valid and returns length. Invalid start
(negative or greater than length) fails E012 **before** byte-needle range checking;
byte needles outside 0..255 fail E014 even on an empty/end-start search. Buffer
needles may be any i64. It allocates nothing and has no host effect. Together with
existing slice/loops it provides explicit delimiter iteration; it creates no views.

`parsebuf` treats each byte in `separators` as an independent separator; order and
duplicates do not matter. An empty set parses the whole nonempty input as one field.
Empty input returns an empty buffer; one terminal separator is allowed. Leading,
interior or repeated final separators produce empty fields and fail E016. Every
field follows `parse`'s canonical decimal i64 rules, including extrema. Separators
are raw bytes and may include NUL/non-UTF-8. There is no implicit whitespace trim.

Count fields first, then reserve exact output capacity (`8 * fields + 40` bytes)
with both input operands live, then parse fields left-to-right. Thus E013 precedes
E016 when quota failure and invalid fields coexist. This order is part of the new
operation's contract. There are no additional diagnostic codes. Arguments still
evaluate left-to-right before any intrinsic check, and lazy unselected arms do
nothing. Parsebuf has no host effect, but its input expressions may have effects.

The output is a fresh immutable buffer. Conversion changes element width, so it
cannot reuse byte input storage. It remains conservative in root tracking. Equality
and find extend the existing allocation-free borrowing summary; neither changes
aliases or liveness. Result buffers compose with existing concat/replacement
last-use and runtime-uniqueness proofs; no copying is reintroduced into a proved
append chain. Retained caller/loop aliases still force copying. See [ADR 022](../adr/022.md).

```text
:v=!parsebuf("1,-2\n3\n",",\n")
:b=!equal(!buffer(2,7),!buffer(2,7))
=!find("one\ntwo",10,0)
```

## Memory and effects

**Quota meaning changed again:** the unchanged 64 MiB live-storage limit now
charges reserved **capacity × element width + 40 bytes** per distinct allocation,
not logical payload + 32. Bytes have width 1; buffers have width 8. Aliases count
once. Dynamic entry arguments, construction, literals, slices and published reads
start with capacity equal to length. Dead SSA values are removed at instruction
boundaries; returned/captured values and active caller/loop state remain roots.
Native collection reclaims unrooted allocations; reference execution deduplicates
shared storage identities.

Concat preserves left capacity when the result fits. Otherwise its capacity is
`max(result_length, min(2 * left_capacity, steady_budget))`, where
`steady_budget = max(0, 64 MiB - 3 * 40 - right_capacity * width) / (2 * width)`
using integer division. This doubles spare space until the steady-state budget
for left, right and result would be exceeded. The required length always wins;
spare capacity never authorizes exceeding the quota. Copying and reused results
use the same rule. Replacement preserves capacity; slicing produces exact capacity.

Each allocating instruction checks the full result capacity while its operands
are still live, including in-place replacement or append with existing room.
Thus optimization does not change E013 or its priority. Spare capacity can make
programs fail E013 earlier than the former payload-only accounting. Discarded
results no longer accumulate charges across iterations.

Values remain immutable. Replacement requires the documented chain/last-use
proof and runtime root uniqueness. Concat may extend a left operand only when it
has no later SSA use, exactly one live root, and differs from the right allocation.
Otherwise it copies. Appending to a retained caller argument or alias preserves
the original. Slices still copy; no views or ownership syntax are introduced.
Bounds, byte-range checks and ordered effects remain at their original instructions.

For the canonical one-byte append loop with no other dynamic roots, the maximum
length is 33,554,372 bytes; the next append fails E013. File-transform limits also
depend on live path lengths and aliases: with the 37-byte output path used in the
boundary check, the maximum is 33,554,353 bytes. These are workload-specific
limits, not guarantees for arbitrary programs. See the validation report for
measured boundary commands.

This is a semantic storage budget, not exact process RSS: root bookkeeping,
allocator overhead and temporary I/O/formatting storage are additional. Remaining
native allocations are released after result consumption. No addresses escape into
NIL. See [the reclamation/reuse invariants](../architecture/DYNAMIC_STORAGE.md).

Read/write/out are explicit host effects and stay inside their selected branches.
Functions containing these operations are not pure. LLVM receives opaque runtime
calls without purity/aliasing promises. Reference `execute_values` denies host I/O;
`execute_values_with_host` accepts an explicit `Host`, such as `FileHost` or an
in-memory test host. Native application executables use the caller's OS permissions;
this runtime is not a security sandbox. Blocking I/O has no deadline guarantee.

| Code | Failure |
|---|---|
| E012 | Index/slice bounds |
| E013 | Negative/oversized allocation or live/transient quota |
| E014 | Byte value outside 0..255 |
| E015 | File/stdout I/O failure |
| E016 | Invalid canonical decimal i64 |
| E017 | NUL in a path |
| E018 | Host I/O explicitly denied |

Instruction/call-depth accounting remains optional in native v5 and always bounded
in the reference evaluator. Bounds and allocation checks are always active.

## Scope

This supports text tools, file transformations and runtime-sized numerical data.
It does not add records, floats, C interoperability, concurrency, recoverable I/O
results, memory views, packages or a full semantic plugin registry. The typed
intrinsic syntax is provisional and does not authorize arbitrary parser extensions.
See [the application plan](../GENERAL_PURPOSE.md) and [ADR 017](../adr/017.md).

The private native application driver permits host I/O by default. Setting
`NIL_DENY_HOST_IO=1` explicitly denies read, write and stdout operations with E018,
matching a denied reference `Host`. NUL paths still report E017 before permission
checks. This process policy is not an OS sandbox or a plugin ABI.

Construction diagnostic priority is negative/invalid length (E013), then byte
range (E014), then quota (E013). Replacement checks bounds (E012) before byte
range and allocation. This ordering also applies when an operation has several
invalid inputs; native and reference execution must agree.

## Ordered keyed values (ADR 023)

V5 alone adds `m` (Bytes -> I64) and `t` (Bytes -> Bytes) signatures.
Maps are immutable values; updates return new values and aliases keep the old
contents. Keys use exact byte equality, including NUL/non-UTF-8. Values are
homogeneous; a byte-valued map contains owned sequences, but map-valued entries,
heterogeneous collections and records are not supported. No field syntax is added.

| Intrinsic | Signature | Result/failure |
| --- | --- | --- |
| `map` | `()->m` | Empty integer map; E013 on allocation failure |
| `bytemap` | `()->t` | Empty byte-valued map; E013 |
| `insert` | `(M,Bytes,V)->M` | Insert absent key; existing key E020 |
| `put` | `(M,Bytes,V)->M` | Insert or replace, retaining an existing index |
| `get` | `(M,Bytes)->V` | Missing key E019; byte result is a fresh copy (E013) |
| `has` | `(M,Bytes)->Bool` | Allocation-free membership |
| `size` | `(M)->I64` | Allocation-free entry count |
| `key` | `(M,I64)->Bytes` | Fresh key copy at insertion index; E012 then E013 |

Here M is m or t, and V is its matching I64 or Bytes value type. Invalid types or
sequence index operations on a map fail statically with E007; arity errors use
E006. Dot field syntax is absent and rejected by the lexer with E001.
All argument expressions execute left to right before operation checks. Then
missing/duplicate/index checks precede result allocation/quota. A trap or host
operation in an unselected lazy arm remains unobservable. None of these intrinsics
has a host effect. Iteration order is first insertion order, independent of hashes;
replacement does not move a key. Native collision resolution compares full keys.

Examples:

```text
=!get(!put(!map(),"answer",42),"answer")
:t=!insert(!bytemap(),"raw","\xFF\0")
:s=!key(!insert(!map(),"first",1),0)
```

Quota includes all live map capacity: the existing 40-byte allocation header,
24 metadata bytes, 48 bytes per entry-capacity slot, 16 bytes per slot for hash
buckets, and owned key/value byte capacity. Entry capacity starts at four and byte
capacity at sixteen; each doubles as needed. The empty map charges 336 bytes.
Insertion/update reserves the complete result charge alongside live operands,
including an old map, even when physical storage is reused. Key and byte lookup
copies also charge ordinary sequence capacity/header. Thus 64 MiB remains the
limit, and geometric capacity plus simultaneous old/result charges can stop a
builder well before 64 MiB of logical keys/values. This adds map charges; it does
not alter the existing sequence quota rule.

Last-use analysis and runtime single-root uniqueness jointly permit storage reuse.
Any live alias forces a copy. Unique insertion grows entry/hash and byte storage
geometrically, with amortized entry overhead plus copied key/value bytes (hash
collision worst cases remain possible). Integer and equal-length byte replacements
can reuse storage. Different-length byte replacement repacks the owned payload,
which is linear in stored bytes; it is not a borrowed view or universally constant
cost. All stored keys/byte values are independent of their source sequences.

The CLI accepts only `{}` for a map entry parameter; construct populated maps in
NIL and pass them between functions. Map results use a tooling representation:
an insertion-ordered JSON array of `[hex-key,integer]` or `[hex-key,hex-value]`
pairs, followed by LF. This output is not a language-level JSON serialization API.

## Deterministic sorting

`!sort(data,order)` accepts `v`, `s`, `m` or `t` and an i64 order, returning
the same collection type. Fixed arrays are excluded. Sequence order 0 is ascending,
1 descending: buffer integers compare signed, bytes unsigned. Map order 0 is
lexicographic byte-key order; order 1 compares values ascending, then keys
ascending. Byte comparisons are unsigned lexicographic, shorter prefixes first.
There is no locale, callback or Unicode comparison. Equal map values are
key-ordered, **not** preserved in insertion order; this complete comparator gives
a deterministic stable observable result. Equal sequence elements have no identity.

Aliases retain their original order. Map lookups preserve associations; subsequent
new keys append to the sorted order. Evaluate arguments left-to-right, then reject
orders other than 0/1 with E012, then reserve the complete result capacity (E013).
This applies even to empty collections and reused storage. Wrong types/arity use
E007/E006. No host effect or other new runtime failure is introduced.

Sorting retains semantic capacity and charges input plus result capacity under the
existing 64 MiB rule. Last-use proof plus runtime single-root uniqueness allows
in-place sorting; live aliases force copying. Native heapsort uses constant scratch
and O(n log n) comparisons, rebuilding map lookup buckets once. Sorting repeatedly
in a loop still repeats that work; reuse removes copying, not comparisons.

```text
:v=!sort(!parsebuf("3,-1,3",","),0)
:m=!sort(!put(!put(!map(),"z",2),"a",1),1)
```

## Snapshot iteration

`!each(input,state*;step*;finish)` iterates a sequence (`v`, `s`, fixed array)
or map (`m`, `t`). Supply 1–4094 user states. Evaluate input once, then initial
states left-to-right. Empty input executes finish directly; otherwise visit every
sequence index or map entry in its current deterministic order. No early exit.

In step, `a` is the sequence index (i64) or map key (owned `s`); `b` is the
sequence element (i64) or map value (i64/owned `s`). `c`, `d`, ... are user
states. Step returns one expression per state, preserving its type. Evaluate
these left-to-right using old state, then commit them simultaneously. Finish
binds only user states as `a`, `b`, ... and returns one value. Nested loops/each
shadow bindings; lazy arms retain the surrounding region's bindings.

The input is an immutable snapshot: replacements/updates of an alias never change
its elements, length or order. For maps, materialize an owned key first, then an
owned byte value if applicable, before the user's step. Both copies can fail E013,
including when their bindings are ignored. Sequence elements/integer map values
allocate nothing. Body/finish traps and effects preserve existing lazy, left-to-right
ordering. Invalid collection/state types use E007; wrong step count E006; unknown
binding E005; malformed syntax E001.

Lowering emits the existing validated HIR Loop with hidden snapshot/index states,
checked element/entry access and index advancement. There is no iterator object,
borrowed view or new execution engine. Append/update user states compose with
existing reuse proofs; aliases of the snapshot require copying. Owned map byte
materialization remains linear in the bytes visited. Earlier profiles/default
reject `sort` and `each`; all previously valid programs retain their meaning.

```text
(v)=!each(a,0;c+b;a)
(m):s=!each(!sort(a,0),"";!concat(c,!concat(a,"\n"));a)
```

See [ADR 025](../adr/025.md) for preregistered corpus forecasts and rationale.

For standalone sort, measured quota boundaries are 33554392 byte elements or
4194299 i64 elements; the next element fails E013. Other live storage reduces
those limits. The decimal-key map builder can insert 262145 entries unchanged,
but sorting its result requires the additional full-capacity reservation and
passes 262144/fails 262145. These are workload-specific boundaries, not a changed
quota or insertion ceiling. [Measurements](../../benchmarks/reports/2026-10-03/V5_ALGORITHMS_PERFORMANCE.json).
