# NIL source standard library

These pure expr-v5 sources are embedded in the compiler and loaded by name only
when referenced. There are no runtime file searches, import flags or independent
std versions. Bodies receive ordinary type validation, static specialization,
borrow/effect analysis, root tracking and live-capacity quota enforcement.

- `!map(&function, bytes_or_buffer)` maps i64 elements to i64 elements.
- `!filter(&predicate, bytes_or_buffer)` retains elements whose callback returns bool.
- `!fold(&function, bytes_or_buffer, initial)` reduces with an i64 accumulator.

Bytes/Buffer overloads have the same spelling. Bytes map results must be 0..255;
Buffer arithmetic remains wrapping i64. Callbacks run in input order; empty input
runs none. Captured state, general record/array/map forms and runtime-selected
functions are not part of these initial signatures. `!map()` remains the existing
integer-map constructor.

```text
:s=!map(&b,"abc")
1=a+1
```

This returns `bcd`. Compile using `nil --profile expr-v5`; the default and earlier
profiles are unchanged. Existing find/has/parse remain core until source migration
can preserve their checked-failure, quota and speed contracts. No wrapper around
core is presented as a migrated implementation.

See [ADR 035](../docs/adr/035.md) and [expr-v5](../docs/language/EXPR_V5.md). Source uses the repository license.

## Comparator sort prototype

`sort.nil-module` explicitly exports stable i64-buffer merge sort as module 7,
operation 0. Load it with `--module std/sort.nil-module`, then call
`!7.0(&b,values)` with `2:b=a<b` as a consistent strict-order comparator.
It is an ordinary source module, not an auto-loaded replacement for core `!sort`.
The merge step invokes its comparator three times per element; effects execute
in source order. Empty inputs invoke no comparator. The prototype uses copying
slices and guarded concat builders and retains ordinary E013 quota failures.
It is a performance baseline for a future library migration, not a parity claim.
