# expr-v4 — typed functions and fixed arrays

Status: implementation experiment extending expr-v3. Earlier profiles and the
expr-v0 default retain their grammar and semantics. Select `--profile expr-v4`.
No claim of best model-generation/TCR representation follows from source counts.

## Semantic core

Types are i64, bool and fixed-length i64 arrays of 0..256 elements. Arrays are
immutable values: indexing returns i64; replacement returns a new equal-length
array without altering aliases. Length is part of the static type. Calls/returns,
branches and simultaneous loop state can carry arrays/bools. Arithmetic remains
v3's wrapping i64, defined MIN/-1 division and division-zero trap. Array indexing
and replacement trap with E012 for negative or out-of-range indices at all
optimization levels, including unbounded execution. Values never contain exposed
pointers. No implicit bool/integer conversion: use comparison or ternary explicitly.

## Signatures and expressions

Existing numeric arity headers mean all-i64 parameters; omitted arity means zero.
Result defaults to i64. `:b` declares bool result; `:N` declares an N-element array.
Mixed/array/bool parameter signatures use parentheses: `i` means i64, `b` bool,
and numeric `N` means an N-element i64 array. All-i64 parenthesized headers and
explicit `:i` are rejected as redundant aliases. Names/parameter slots remain
positional; one function per line, forward calls supported.

```text
(8,i)=a[b]
(8):8=a[0:1]
1:b=a>0
(b,i)=a?b:0
```

The first reads one array element, the second returns a replaced array, the third
returns a predicate and the fourth accepts a bool and integer. Each line is a
separate function with declaration-order reference a/b/c/d.

- `[x,y,...]`: array construction; empty `[]` is array length zero.
- `[x;N]`: evaluate x once and repeat N times, N must be 2..256. This differs from
  repeated expressions when calls/traps/resource accounting occur.
- `#a`: array length; postfix indexing binds before length.
- `a[i]`: checked read; `a[i:v]`: checked immutable replacement.
- Existing arithmetic, comparison, calls, lazy ternaries and `@(...)` loops remain.

Array literals evaluate elements left-to-right. Replacement evaluates array, index,
then value and checks bounds; lazy branches do not evaluate unselected operations.
Loops update every state value simultaneously. Source/depth and aggregate limits
apply before execution; bounds guards are semantic and cannot be disabled by budgets.

## Entry interface and lowering

Native typed entries marshal flat decimal i64 slots in declaration order. An array
consumes N slots; bool consumes one slot restricted to 0/1. Results print decimal
integers, true/false, or JSON integer arrays. Internal calls remain fully typed.
A generated LLVM bridge marshals slots, avoiding dependence on C struct/LLVM
aggregate calling-convention equivalence. Legacy all-i64 ABI/driver is preserved.

HIR adds construct, repeat, length, read and replace operations. The evaluator
uses immutable shared array values. LLVM uses aggregate values and typed phi joins;
dynamic indexing uses guarded private storage, with allocations in the entry block.
Proven single-use loop replacements use private reusable storage, including
guarded chains. Writes commit after body evaluation without changing immutable
semantics. Other chains retain aggregate copies. Typed functions are internal
behind the entry bridge and receive an inlining hint; this is not a public C ABI.
See [ADR 015](../adr/015.md). No escaping addresses, raw pointer grammar or
language-level allocation is added.

## Acceptance and postponed work

Acceptance: sum/dot/max, binary search, array transformation, typed bool/array calls,
empty arrays, alias preservation and bounds failures agree in evaluator/native
O0/O2; invalid signatures/operands/HIR are rejected. Compare equivalent algorithms
against Python/C++ under frozen bounded numeric domains, including source context,
actual tokenizer counts, compilation stages, runtime and binary size.

Extra widths, floats, heterogeneous records, dynamic arrays, references/pointers,
FFI, persistent globals and plugin operations remain unselected. Fixed arrays are
not a final systems-memory design. If copying dominates, measure lowering first;
introduce references only after a documented ownership/aliasing requirement.

## Running the examples

```sh
cargo build --release -p nil --locked --offline
./target/release/nil --profile expr-v4 run examples/expr-v4/sum.nil 0 1 2 3 4 5 6 7 8
# 36
./target/release/nil --profile expr-v4 run examples/expr-v4/reverse.nil 0 1 2 3 4 5 6 7 8
# [8,7,6,5,4,3,2,1]
./target/release/nil --profile expr-v4 run examples/expr-v4/predicate.nil 0 -3
# false
./target/release/nil --profile expr-v4 build examples/expr-v4/binary_search.nil -o /tmp/nil-search
/tmp/nil-search 1 3 5 7 9 11 13 15 9
# 4
```

The number after the source path in `run` is the entry function ID, not its first
argument. Compiled executables take only flattened arguments. See all eleven
programs under `examples/expr-v4`; the benchmark corpus also covers lengths up to 256.
