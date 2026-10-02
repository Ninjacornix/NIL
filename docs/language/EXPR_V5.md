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

Lengths must be nonnegative; slice start/length must fit the sequence, including
empty slices at its end. Decimal parsing accepts only canonical i64 strings:
no whitespace, leading plus, leading zeroes, negative zero, or trailing newline.
File paths are OS bytes and cannot contain NUL. Writes create/truncate their target;
failures may leave partial output. No atomic-write guarantee is provided.

## Memory and effects

Each execution charges dynamic payload plus 32 bytes per allocation against a
64 MiB cumulative limit, including dynamic entry arguments. The quota counts
allocations even after a reference value becomes unreachable. It is an execution
policy, not a claim about exact process RSS; temporary I/O/formatting storage is
additional. Native allocations belong to an execution arena and are freed after
result consumption. No addresses escape into NIL. Copying replacements/slices is
intentional initially; large update loops can exhaust the quota.

Read/write/out are explicit host effects and stay inside their selected branches.
Functions containing these operations are not pure. LLVM receives opaque runtime
calls without purity/aliasing promises. Reference `execute_values` denies host I/O;
`execute_values_with_host` accepts an explicit `Host`, such as `FileHost` or an
in-memory test host. Native application executables use the caller's OS permissions;
this runtime is not a security sandbox. Blocking I/O has no deadline guarantee.

| Code | Failure |
|---|---|
| E012 | Index/slice bounds |
| E013 | Negative/oversized allocation or cumulative quota |
| E014 | Byte value outside 0..255 |
| E015 | File/stdout I/O failure |
| E016 | Invalid canonical decimal i64 |
| E017 | NUL in a path |
| E018 | Reference host I/O not enabled |

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
