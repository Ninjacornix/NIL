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
| `!format(value)` | numeric scalar | specified numeric bytes (see below) |
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

## Scalar numeric types (ADRs 026–027)

Opt-in v5 adds `u64`, `u128` and IEEE-754 binary64 `f64` scalars. Existing `i`
remains signed i64. No other widths or float/unsigned collections are added.
Unsigned integer literals are canonical decimal digits followed by `u64` or
`u128`: `18446744073709551615u64`. Unsuffixed integers remain i64. Float literals
have decimal digits and a fractional part or exponent: `1.0`, `-0.0`, `1e-3`.
Fractions/exponents require digits; literals must be finite (E001 otherwise).
Nonfinite values are constructed by parsing or bit construction. These spellings
are unavailable in earlier profiles; the default remains expr-v0.

```text
(u64,u128,f64):f64=!f64(a)+!f64(b)+c
:u64=18446744073709551615u64+1u64
:u64=!bits(-0.0)
```

Arithmetic and comparisons require identical numeric operand types (E007 for
mixed types). Unsigned add/subtract/multiply wrap modulo 2^width; unsigned division
truncates and traps E009 on zero. Signed i64 behavior is unchanged. Binary64
add/subtract/multiply/divide round **each operation** to nearest, ties to even,
with gradual underflow. Float zero division yields signed infinity or NaN and
never E009. No fast math, reassociation, reciprocal approximation, fused
multiply-add or observable rounding environment. All native compilation and
link stages use `-fno-fast-math -fno-associative-math -fno-reciprocal-math
-ffp-contract=off`; LLVM operations carry no fast-math flags.

All NaNs entering or produced by execution normalize to positive quiet bits
`0x7ff8000000000000`. Payload, sign and signaling status are not observable.
Equality involving NaN is false, inequality true, ordered comparisons false.
`-0.0 == 0.0`, but bits, formatting and division distinguish their signs. Infinity
behaves as IEEE-754 specifies. Map keys remain byte strings, never numeric values;
explicitly formatted signed zeros are distinct byte keys.

| Operation | Input | Result / behavior |
|---|---|---|
| `!i64(x)` | numeric scalar | checked signed i64 |
| `!u64(x)` | numeric scalar | checked unsigned 64-bit |
| `!u128(x)` | numeric scalar | checked unsigned 128-bit |
| `!f64(x)` | numeric scalar | binary64; integers round nearest/ties-even |
| `!trunci64(x)` | integer scalar | low 64 bits interpreted as two's complement |
| `!truncu64(x)` | integer scalar | low 64 unsigned bits |
| `!bits(x)` | f64 | canonical representation as u64 |
| `!floatbits(x)` | u64 | f64 with NaNs normalized |
| `!parseu64(text)` | s | canonical decimal u64 |
| `!parseu128(text)` | s | canonical decimal u128 |
| `!parsef64(text)` | s | binary64 from specified ASCII grammar |
| `!format(x)` | i64/u64/u128/f64 | owned byte sequence |

Checked conversions fail **E021** when outside the destination interval. Float
to integer requires finite input in `[−2^63,2^63)`, `[0,2^64)` or `[0,2^128)`,
respectively, then truncates the fraction toward zero. Guards precede the cast;
NaN/infinity cannot form LLVM poison or an undefined C cast. Explicit truncating
conversions accept only integers and have no range trap. No implicit conversion.

Unsigned parsing accepts digits only, no sign or leading zeros except `0`, within
the target range. Float parsing accepts exactly `nan`, `inf`, `-inf`, or
`[+-]?[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`. Reject whitespace, NUL, hex notation,
partial fractions/exponents and trailing bytes with **E022**. Decimal rounding is
nearest/ties-even; overflow yields infinity, underflow may yield signed zero or a
subnormal. Existing `!parse` remains canonical i64 with its unchanged E016 code.

Unsigned formatting is canonical decimal. Float formatting uses **17 significant
digits in fixed scientific form**, one leading digit and 16 fractional digits,
lowercase `e`, decimal exponent without plus or leading zeros. Examples:
`1.0000000000000001e-1`, `0.0000000000000000e0`,
`-0.0000000000000000e0`; special text is `nan`, `inf`, `-inf`.
Parsing formatted output recovers the canonical **exact bit pattern**. This is
intentionally not shortest formatting. Reference Rust and native C text conversion
are parity-gated; no approximate oracle is used.

Arguments evaluate left to right before the operation's checks. Conversion/parse
failures occur at that operation, remain lazy in unselected arms, and preserve
host-effect/trap ordering. Arithmetic/conversion/bits/parsing have no semantic
allocation or host effect; native parsing scratch is outside live-capacity storage
as in ADR 017. Format allocates owned bytes, charged against unchanged 64 MiB live
capacity, failing E013 if admission fails; aliases and root rules remain unchanged.
Native entries take one decimal argument per unsigned or float parameter (`u128`
also takes one CLI argument, although its private bridge uses two words). Float
entry text follows parsef64 grammar. Invalid entry arguments fail E010. Results
use the same numeric formatter plus the normal result newline. This private bridge
is not a plugin ABI.

[ADR 026](../adr/026.md) defines the boundary: participation in allocation/alias/
reuse/effect proofs requires compiler-visible contracts, but does not make every
algorithm a permanent core opcode. Fundamental numeric/value/control/storage
semantics stay core; algorithms may later move to validated compiler-visible
extensions that preserve every contract. No migration or stable ABI is implemented.

## Nominal records (ADR 028)

Declare records before functions; definitions reference only earlier definitions.
Fields have unique identifier names, declaration order and fixed static types.
Record names start with an uppercase ASCII letter. Existing profiles and default
syntax are unchanged.

```nil
record Point(x:i,y:i)
record Message(point:Point,text:s)
= b(Message(Point(20,22),"answer"))
(Message)=a.point.x+a.point.y
```

A record name is a signature type: `(Point):Point=a{x:a.x+1}`. Construction
`Point(20,22)` supplies all fields once in declaration order. `value.x` projects a
field. `value{x:expression}` makes a new value changing exactly that field.
Receiver precedes replacement evaluation; constructors evaluate fields left to
right. Lazy arms stay lazy, including field expressions with effects or traps.
There are no defaults, destructuring, reflection, serialization, comparison or
record formatting operations. ADR 026 rejects convenience algorithms as new core
operations; construction/projection/update are fundamental product primitives.

Records use immutable LLVM SSA aggregates. Nested records, scalar/fixed-array
fields and existing sequence/map fields are supported. Dynamic fields share their
immutable allocations; updates never mutate an old record or its shared payload.
Every live dynamic leaf is rooted, including repeated aliases. This prevents
sequence/map unique-update paths from overwriting a value still accessible through
a record. Nonallocating record helpers may borrow; allocating regions retain
conservative roots. A direct Bytes/Buffer field projection may transfer its root
when it feeds one concat immediately followed by overwrite of that field, the old
field cannot be read or escaped again, and no allocation intervenes before the
last other-field read of the old record. Other field/caller/parent aliases still
force copying. Unproved field chains keep whole-record liveness and can copy per
iteration. See [ADR 031](../adr/031.md); observable immutable semantics are unchanged.

Inline record storage has no capacity and no dynamic-arena charge, like existing
fixed arrays. All contained allocations count once by identity at their full live
capacity plus existing headers, even through nested records. No quota exemption
for dynamic fields and no arena layout change: the 64 MiB limit is unchanged.
LLVM owns private aggregate layout/padding; it is not a C or plugin ABI. Limit
expansion to 128 declarations, 1..64 fields per record, 32 nesting levels
and 4096 flattened slots. Recursive/forward definitions are rejected.

Scalar-record maps use type `map[Point]`, constructed with `!map[Point]()`. Existing
`!insert(map,s,Point)->map[Point]`, `!put(map,s,Point)->map[Point]`,
`!get(map,s)->Point`, `!has`, `!size` and `!key` retain their signatures and ordering.
Records with only scalar or fixed-i64-array leaves, including nested such records,
are supported as map values. Private packed values use 8-byte little-endian slots;
u128 uses low then high, f64 stores canonical bits and bool uses 0/1. Lookup
allocates nothing. Insert/update preserves aliases and uses the existing unique
map proof and live-capacity reservation. Map storage charges
`40 + 24 + 64*entry_capacity + byte_capacity`, including packed record bytes.
Insertion order is deterministic. `!sort(map,0)` sorts byte keys; `!each` exposes
byte keys and typed record values. Value-order mode 1 fails E018 before quota;
orders outside 0..1 still fail E012 first. Sequence-bearing record map values and
record buffers are deferred: their child ownership/element contracts are not
supplied by packed scalar storage. No implicit serialized emulation.

E023 (Check) reports unknown/duplicate record or field, invalid definition/layout,
or unsupported aggregate collection placement. E006 reports constructor arity,
E007 reports field type mismatch, and existing E008 input-slot limits still apply.
Static diagnostics precede execution. Projection/update add no runtime failures;
existing field-expression failures retain left-to-right ordering. Record maps
retain E019 missing key, E020 duplicate-before-quota, E013 live-capacity quota and
existing effect/fuel/depth checks. Native command-line entry marshalling does not
yet accept record or record-map arguments/results (E010); wrap internal record
calls with supported entry types, as above.

The intrinsic enum remains 32 variants (33 application operations when counting
`each` lowering). Records add three HIR product operations and a typed map
constructor operation/overload, not another algorithm library. F64 formatting
remains ADR 027's fixed scientific format; shortest-round-trip is only a proposal.

## Semantic plugins (ADR 029, interface version 1)

`!plugin(plugin_id,operation_id,arguments...)` invokes a statically typed export
from an explicitly loaded local manifest. IDs are canonical u32 integer literals,
not runtime expressions. No plugin defines parser syntax. Signatures use every
existing type and the caller's nominal record registry, including nested records,
sequences, maps and current scalar-record map limits. LLVM layout stays private;
this versioned semantic linking interface is not a stable C/binary ABI.

Version 1 requires a `borrow` declaration independently proved from validated
provider HIR: no dynamic-arena allocation or host effect. Unknown/allocating/host
contracts, recursion and nested plugin providers are rejected. Providers cannot
add/redefine caller record types. Arguments and returned aliases retain ordinary
arena allocation identities, roots and live-capacity charges; only the core
runtime frees storage. Static record construction/projection/update can return
borrowed dynamic fields. No address escapes into NIL. Allocation/reuse or
unselected-arm effects cannot hide behind a declaration.

Arguments evaluate once left-to-right; inactive arms execute nothing. E006/E007
report arity/type mismatch and E023 record errors. E024 identifies invalid manifests,
versions/exports/effect contracts or registry mismatches. Executed provider operations
retain their existing diagnostic codes/trap ordering. Calls consume one caller
instruction and no extra caller depth; their internal finite library loops are not
charged as user HIR loops. Reference internal limits are u64::MAX steps/depth 256;
native providers use no caller instrumentation. Providers are acyclic with at most
128 functions. This does not guarantee termination of trusted arbitrary loops.

`!plugin(0,0,lhs,rhs)` addresses shipped equality without a manifest.
`!equal(s,s)->b` and `!equal(v,v)->b` keep their existing spellings and semantics,
but execute the shipped sequence plugin. The old HIR intrinsic enum is accepted
only as a compatibility input normalized to a typed plugin call. There are still
32 enum variants, **31 executable core intrinsic implementations plus this adapter**;
no second intrinsic migrated. Plugin-only `bump(Packet)->Packet` in the worked
example updates `count` with wrapping i64 addition and preserves every other field,
including dynamic aliases. It allocates nothing and introduces no runtime failure
of its own; argument traps and typed/link errors remain observable as specified.

Loading is compile-time and local via `--plugin MANIFEST` or the Rust linking API.
No native object/dylib loading, runtime dispatch or remote loading is implemented.
**There is no sandbox.** Trusted source can consume CPU; compiler/runtime bugs and
host effects remain risks. See [the worked authoring guide](../architecture/PLUGINS.md)
for manifest syntax, commands, reference/native execution and deferred owned/effect
providers. Earlier profiles and the expr-v0 default are unchanged.

## Dynamic record buffers (ADR 030)

`v[Name]` is a homogeneous dynamic collection of a previously declared record:

```nil
record Edge(to:i)
record Row(edges:v[Edge])
(v[Row]):i=#a
```

`!buffer[Name](length,fill)` constructs `v[Name]`; fill has nominal type `Name`.
Length is i64. Arguments evaluate once, left-to-right (including fill before the
constructor's length/quota checks). Negative/overflowing length or quota fails E013.
Malformed constructor syntax/operand count fails E001; unknown/forward/cyclic types
fail E023, wrong typed operands E007. Existing record construction arity is E006.
There is no default fill or recursive type. Type IDs decrease through nested
record-buffer fields; integer indices, conventionally -1 for absent links, are
ordinary checked values and can represent cyclic graph edges without cyclic values.

Existing operations specialize without new intrinsic names:

| Form | Result / contract |
|---|---|
| `#xs` | i64 record count |
| `xs[i]` | immutable `Name`, checked E012 |
| `xs[i:value]` | `v[Name]`, same nominal element type; E012 before quota |
| `!concat(xs,ys)` | same typed buffer; overflow/quota E013 |
| `!slice(xs,start,count)` | owned outer buffer, shared immutable dynamic children; E012 range before E013 |
| `!each(xs,state;step;result)` | snapshot in index order; element is `Name` |

Receiver/index/value and intrinsic arguments evaluate once left-to-right; an
argument trap precedes the operation's own checks. Lazy unselected arms stay inactive.
Sort over record buffers is **deferred**: invalid mode fails E012, valid mode 0/1
fails E018 before allocation. No arbitrary lexicographic record comparison is supplied.
Records may contain ordinary sortable integer buffers or maps.

Records can contain buffers of earlier records and existing sequence/map fields.
Wrappers such as Row(edges:v[Edge]) give nested dynamic adjacency. Existing scalar
record map values stay supported; sequence-bearing record map values, generic
sequence-valued maps and map-valued map entries remain deferred. Plugins cannot
define types; providers using this type require `types record-buffer` (E024 otherwise).

### Immutable aliases, layout and quota

Index/projection returns immutable aliases of child values. Replacement/concat/slice
preserve all old aliases. Packed rows use 8-byte slots (u128 two slots), including
private arena handles; the static descriptor lists dynamic slots. The native header
is 40 bytes; one metadata row of width W precedes capacity rows. Charge is exactly
**40 + (capacity+1) * W**, W = 8 * max(1, record logical slot count). Child allocations are separately
charged once per allocation identity, recursively; unused outer capacity is charged.
The unchanged 64 MiB limit counts live capacity, including copy/growth reservation
peaks. The reference uses this semantic charge, not its larger Rust heap layout.
No layout/pointer is exposed to NIL and no stable foreign ABI is implied.

A rooted outer buffer retains each child edge transitively; dropping the last outer
root releases these edges. Repeated child handles retain alias multiplicity. Strict
acyclic type order bounds traversal. Existing arena collection frees dead storage;
this is an extension of shadow roots, not user-owned reference counting or GC.
Static last-use plus runtime unique root checks permit in-place row replacement
and geometric concat growth. A child retained by another parent/extracted value
prevents false uniqueness. Replacement retains new children before dropping old.

Scalar-record builders amortize geometric capacity and single-use updates reuse
outer storage. Dynamic-child root transitions can traverse all populated rows;
nested builders are not promised linear. Record-field append remains conservative
and can copy quadratically. Algorithms, record ordering and serialization belong
to future libraries under ADR 026; intrinsic variants stay 32.

Zero-slot records (for example `record Z(x:0)`) remain valid. Their record-buffer
rows and metadata each occupy one initialized 8-byte physical word, although
field access still yields zero-length arrays. Packed record-map values keep
their logical zero-byte encoding. Zero-width buffer division is never permitted.


### Proven bulk comparison and retained builder roots

Native lowering can recognize a validated provider's complete length-guarded,
unit-stride equality scan over Bytes/Buffer and use a nonallocating bulk comparison.
Recognition depends on HIR semantics, not provider identity; reference execution
continues to run the provider body. Altered bounds, strides, exit results or extra
potentially trapping accesses retain ordinary provider lowering. Argument order,
traps, laziness, caller fuel/depth and borrowing/effect proofs are unchanged.

A final single-use concat of loop state can retain its parent root across the
backedge, preserving child edges without rescanning them. Active aliases still
select copying. Native collection of dead arena entries may be deferred while the
next physical reservation already fits 64 MiB; it runs before a reservation would
exceed that cap, or garbage reaches `max(64 KiB, live capacity / 4)`. This internal
scheduling budget keeps dead copied payloads from accumulating indefinitely.
Programs proven to contain no record buffers retain eager flat-arena reclamation;
there are no transitive heap child edges in those programs. Live-capacity accounting and copy-equivalent quota reservations
are unchanged; temporary dead storage cannot raise the physical cap. Allocating
nested regions and escaping calls remain conservative. These are compiler/runtime
optimizations, not syntax, ownership or quota-contract changes.

## Local source modules (ADR 032)

General modules use the plugin source loader, not a second parser or a native ABI.
A manifest contains:

```text
nil-module 1
id 7
source reports.nil
export 0 0
export 1 1
import shared.nil-module
```

`import` is optional and repeatable; other required declarations occur once.
Exports map canonical numeric operation IDs to local positional function labels.
Only exported functions are callable across files, using the existing
`!plugin(7,0,args...)` expression and its exported typed signature. There is no
new intrinsic, type or punctuation. Local `a(...)`, `b(...)` calls keep their
existing meanings. Argument evaluation remains once, left-to-right.

Load root imports explicitly with repeatable `--module MANIFEST` (an alias of
`--plugin MANIFEST`) after `--profile expr-v5`, or use Rust
`compile_with_plugins(source, SourceProfile::ExprV5, paths)`. Each imported module
sees only its direct imports; transitive imports are linked but not automatically
re-exported. Dependencies shared by multiple roots load once by canonical path.
Different manifests claiming one ID, duplicate imports/exports, missing exports,
unknown versions/directives and import cycles fail **E024**. ID 0 stays reserved.
Relative source/import paths cannot contain parent traversal or be absolute.
Sources are bounded UTF-8 regular files; symlinks are not a sandbox boundary.

Module calls become ordinary HIR calls. Allocation, host effects and local self/
mutual recursion are allowed, with the same caller arena, immutable values,
live-capacity quota, call-depth/fuel limits, trap order and conservative roots as
local functions. Imported allocating calls do not acquire a borrowing exemption.
Nonallocating calls participate in existing interprocedural proofs. Calls across
an acyclic dependency graph are allowed; mutually importing modules are rejected.
`nil-plugin 1` still requires proved borrow-only, nonrecursive, unnested providers
and retains its atomic caller accounting. Module declarations cannot grant purity.

All modules use the root's complete nominal record registry; they cannot add or
redefine types in version 1. Body/type failures preserve existing E0xx codes;
wrong arity/type are E006/E007, unknown local functions E004, duplicate local
labels E003. Modules are snapshotted and validated before execution, so later file
changes cannot change a compiled program. Source spans are still local offsets
(including injected record declarations), not globally unique file locations.

Bounds: 128 distinct manifests, import depth 64, 128 functions per imported module,
4096 linked module/root functions, 128 exports per manifest, and 1 MiB aggregate
source including root and injected registries. Individual manifests obey the
existing 1 MiB file limit. These limits fail E024; normal parser limits still apply.
Function 0 in the root remains the default entry; explicit entry labels refer only
to root functions. Imported labels are private even after relocation.

See `examples/expr-v5/modules/` for shared reporting code. Qualifying an imported
reference costs tokens; single-file sources need no manifest or changed spelling.
This is trusted local source linking, with no sandbox, remote loading or package
manager. No user address crosses the boundary.

## Host environment, entropy and directory effects

ADR [033](../adr/033.md) adds three ordered host operations only in expr-v5.
They are not borrowing/pure operations. Modules execute them using the caller's
Host; borrow-only source providers reject them. All operands evaluate left to
right. Without an explicitly supplied reference Host, each reports E018.

| Operation | Signature | Result and failures |
|---|---|---|
| `!env(name)` | s -> s | Owned OS-byte value; absent and empty both return empty. Empty name, NUL or `=`: E017 before permission; denied: E018; acquisition: E015; result quota: E013 |
| `!random()` | () -> u64 | Advances the supplied Host's entropy state once. Denied: E018; invalid injected seed: E016; entropy acquisition: E015 |
| `!directory(path)` | s -> m | Owned insertion-ordered map of immediate basename -> 0, sorted by unsigned byte lexicographic order. NUL: E017 before permission; denied: E018; missing/unreadable/non-directory or acquisition failure: E015; result quota: E013 |

Directories include files, subdirectories and symlinks, excluding `.` and `..`.
Names are not joined paths, entries are not followed, and metadata is not returned.
No recursive enumeration, globbing, environment mutation or recoverable I/O is
provided. Unix OS bytes are preserved; filesystem restrictions still apply (this macOS
filesystem rejected the non-UTF-8 filename fixture). An injected Host must supply valid distinct basenames;
duplicate snapshot names report E020. Results are immutable snapshots, not promises
of an atomic snapshot during concurrent filesystem modification. Existing map
capacity growth and live-capacity charges apply, including 40-byte allocation and
24-byte map headers, 64 bytes per reserved entry and reserved key-byte capacity.
Successful acquisition precedes final quota admission; no host pointer escapes.

Randomness is **not cryptographic**. `SeededHost::new(inner, seed)` supplies
SplitMix64 state and forwards other Host methods without granting their permissions.
Native tests inject the same canonical decimal u64 using `NIL_RANDOM_SEED`.
The state transition is wrapping add `0x9e3779b97f4a7c15`, xor-shift 30 and multiply
`0xbf58476d1ce4e5b9`, xor-shift 27 and multiply `0x94d049bb133111eb`, xor-shift 31.
Seed zero starts with `0xe220a8397b1dcdaf`, then `0x6e789e6aa1b965f4`.
Native permission is checked before seed parsing. Invalid seeds trap only on a
selected draw. Unselected lazy arms neither acquire entropy nor advance state.
`FileHost::default()` and native execution without injection seed lazily from
`/dev/urandom` on supported Unix hosts; no clock or fixed-value fallback exists.
Native execution cleanup resets RNG state. Exact reference/native comparisons
require the same injected seed, never two independently acquired OS seeds.

`FileHost` is now stateful: Rust callers replace the former unit value `FileHost`
with `FileHost::default()`. Default reference execution still denies all host
capabilities. Filesystem/native execution remains trusted and unsandboxed.

```text
(s):s=!env(a)
:u64=!random()
(s)=!size(!directory(a))
```

These are separate programs. Build with `nil --profile expr-v5 build FILE -o OUT`;
run `NIL_RANDOM_SEED=0 ./OUT` for reproducible draws. Seed injection is execution
configuration, not new NIL syntax. Earlier profiles and the default are unchanged.

## Higher-order functions (static specialization)

Function arguments are compile-time references, not runtime values. In an expr-v5
signature, `[T0,T1:Result]` denotes a first-order callback over existing value
types; `[:Result]` takes no arguments. The result type is mandatory, including
`i`. Callback signatures cannot themselves contain callback types. Only function
parameters can have this type: no function-valued field, collection, return,
conditional result or loop state exists.

```text
=b(&c,20)
([i:i],i)=^a(b)
1=a+1
```

This returns 21. `&c` always references local function c. `^a` refers to the
callback in position a of the enclosing function signature; `^a(value)` invokes
it and `^a` alone forwards it to another HOF. Bare `a(...)` retains its existing
meaning as a local function call. Ordinary parameter letters retain their meaning.
Inside loop/each regions, caret names still denote **enclosing function callback
positions**, whereas bare letters denote the existing region state/element slots.
Captured state must be passed as explicit typed value arguments:

```text
:s=b(&c,"prefix","x")
([s,s:s],s,s):s=^a(b,c)
(s,s):s=!concat(a,b)
```

No anonymous functions, implicit captures, dynamic callback selection or function
pointers are supported. Each static callback tuple produces a specialized copy
with ordinary direct HIR calls and only runtime value parameters. Pure/borrow,
allocation, alias, escape and effect facts come from the actual callback body via
existing analyses; no declaration grants a proof. Call argument effects stay
left-to-right. Specialization has no user execution or host effects. Both lazy
arms are checked and may generate code, but only the selected arm executes.

Source modules may export HOF templates through existing manifests and receive
caller-local references in `!plugin(ID,EXPORT,&local,values...)` calls. Conversely,
`&!plugin(ID,EXPORT)` references an ordinary first-order module export. Ordinary
module relocation/import visibility/type rules apply before specialization.
`nil-plugin 1` borrow-only providers reject function-typed parameters with E024;
use `nil-module 1` for higher-order libraries. No new intrinsic is added.

Wrong arity: E006; wrong callback/value/result signature or attempted function
escape: E007; unknown local reference: E004; unknown callback parameter: E005;
malformed syntax: E001/E002 with byte spans. Uninstantiated templates cannot be
runtime entries; their syntax and explicit references are checked, and value types
are checked when instantiated. Runtime recursion keeps the existing call-depth
checks (E008 under bounded execution). Static recursion reuses the reserved
specialization for an identical tuple. Specialization limits are 1,024 globally,
256 per template, 4,096 emitted functions and 1,048,576 copied AST instructions,
including lazy/nested regions; exceeding a limit is E008 at the requesting call.
See [ADR 034](../adr/034.md). Earlier profiles/default and existing program meanings
are unchanged.

Import graphs must remain acyclic, but a passed callback may call back into an
imported HOF, creating a **runtime** call-graph cycle without an import cycle.
Those are ordinary direct HIR calls, handled by the same recursive borrowing
fixed point and bounded call-depth checks. Static callback positions are not
runtime captures and cannot escape. A bare callback parameter used as a value is
E007; use its caret reference when forwarding it.

## Compiler-versioned source standard library

Expr-v5 automatically resolves the reserved names below to shipped pure NIL source
in `std/bytes.nil` and `std/buffer.nil`, embedded with the compiler. Programs need
no import, manifest or flag. The ordinary module/HOF linker and validation produce
first-order direct calls; source bodies, including actual callbacks, determine
allocation, borrowing, effects and roots. There is no std trust exemption or runtime
filesystem search. Std and compiler versions move together. External exports cannot
shadow std/core names. Earlier profiles and the default do not load std.

| Call | Signature | Allocation and failures |
|---|---|---|
| `!map(&f, sequence)` | callback `[i:i]`; Bytes→Bytes or Buffer→Buffer | Exact-length output; E013 quota; Bytes callback result E014 unless 0..255 |
| `!filter(&f, sequence)` | callback `[i:b]`; same input/output sequence type | Owned geometrically growing output; E013 quota |
| `!fold(&f, sequence, initial)` | callback `[i,i:i]`; initial/result i64 | No library allocation; callbacks may allocate/effect/trap |

Sequence forms currently mean Bytes and dynamic i64 Buffer, not fixed arrays,
record buffers or maps. Callbacks receive elements as i64, in ascending index
order. Fold passes accumulator then element. Empty sequences invoke no callback;
fold returns its initial value. Inputs and old aliases remain immutable. Argument
expressions execute left-to-right before traversal; each callback finishes before
the next element. Callback failures/effects remain observable in that order.
Map reserves its result before callbacks; filter reserves its empty builder before
callbacks and appends only selected elements, retaining existing capacity/quota
rules. No unchecked byte conversion or quota bypass exists. Wrong arity is E006,
wrong sequence/callback signature E007; reference errors and specialization limits
retain ADR 034 codes. The unselected arm of a conditional executes no std body.

`!map()` still constructs an integer map; `!map[Record]()` still constructs a record
map. Only the two-argument form denotes the HOF algorithm. `!find`, `!has` and
`!parse` retain their existing core implementations: the source migration candidates
cannot yet preserve all checked-failure/quota contracts. A core wrapper is not
counted as a migration. Intrinsic variants remain 35, executable core bodies 34
(the existing `equal` provider remains outward).

Numeric qualified calls `!7.0(args...)` and references `&!7.0` are aliases for
`!plugin(7,0,args...)` and `&!plugin(7,0)`. IDs/exports must be canonical unsigned
integers. They use exactly the same explicit local manifests, visibility, duplicate
ID rules, snapshots, type checks and E024 failures. Multiple imports cannot collide
silently. No arbitrary module-name binding, remote fetch or auto-loaded third-party
code is introduced.

See [ADR 035](../adr/035.md), which supersedes ADR 026's placement rule. Algorithms
move outward only with equal named-call token costs and measured O2 parity; primitive
storage/scalar/bit contracts and host effects stay core. [ADR 036](../adr/036.md)
specifies guarded callee ownership transfer and scalar snapshot scheduling.

### Guarded ownership and builder lowering

[ADR 036](../adr/036.md) is accepted. This changes native implementation, not
immutable-value semantics, syntax, instruction order, quota, or failure codes.
An argument dead at a call can surrender its root before the callee roots it;
allocation-free transfer gaps cannot collect it. Final-use replacements inside
ordinary callees now use the same guarded path as loop replacements. Active
caller aliases, parallel parameters, returned aliases, and aggregate child edges
keep their roots. Update still copies unless the allocation has exactly one root.
Every replacement reserves the prospective copying charge even when reused.

A bounded scalar snapshot recognizes a later final read of the original at the
same replacement index. Replacement bounds, byte-range and quota checks execute
first at their original span; then the old element is captured before writing.
Only intervening nontrapping scalar work is admitted. Calls, division,
allocation, nested regions and host effects prevent this optimization. The later
instruction retains its original instrumentation tick. Record-valued snapshots
are excluded because their children need independent lifetime proofs.

An adjacent, single-consumer `!bytes(1,x)`, `!buffer(1,x)` or one-byte literal
feeding final-use concat can use activation-local temporary storage. It retains
its header-plus-capacity charge, byte check, source span and instruction position
until concat consumes it. A private virtual root carries that live charge without
an escaping pointer or shadow-stack slot; consumption releases the charge. It
never escapes or enters the heap allocation list.
Concat keeps its ordinary growth, alias guard and prospective quota reservation.
Root-frame headers also use activation-local storage, with identical root slots
and links and distinct storage in every recursive activation.

`std/sort.nil-module` is an explicitly loaded comparator merge-sort prototype;
it is not auto-loaded under `!sort`. Export 0 accepts `[i,i:b]` and `v`, returns
`v`, and preserves the order of equal elements when given a consistent strict
ordering. Its source invokes the comparator three times per merged element;
callbacks execute exactly in that written order. Core `!sort` remains unchanged.
This prototype makes no core-migration or performance-parity claim.

A canonical nonnegative unit-step loop may read an identity-carried flat input
through a cached data pointer while retained roots protect that input across
output allocations. The loop condition proves each admitted index is below its
unchanging length. A writable state initially aliasing the input must copy,
because the input's independent root remains live. Shifted/computed reads,
modified sequences and unproved lifetimes retain checks. This input-read proof
does not discharge writable-output uniqueness or its quota/bounds checks.
