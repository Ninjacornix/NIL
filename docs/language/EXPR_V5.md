# expr-v5 — application data and host operations

Experimental, opt-in with `--profile expr-v5`. Earlier profiles and the default
remain unchanged. This extends v4's wrapping arithmetic, lazy branches, ordered
argument evaluation and immutable values. Source token and generation efficiency
have not been measured for this profile.

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
