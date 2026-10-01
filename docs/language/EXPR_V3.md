# expr-v3 — compact machine-integer expressions

Status: opt-in experimental profile. Select `--profile expr-v3`; the repository
still defaults to expr-v0. See [ADR 013](../adr/013.md) for the rationale and tradeoffs.

## Surface and types

The grammar is exactly [expr-v2](EXPR_COMPACT.md#iteration-2-expr-v2): one function
per line, optional positive arity, positional parameters, declaration-order calls,
infix arithmetic/comparisons, lazy ternaries and `@(...)` state-tuple loops.
Absence of arity means zero parameters. Every function parameter/result is i64;
bool is an intermediate/loop-state type. There is no general type inference.
The profile is compiler input metadata, not an extra token in every generated program.

```text
=b(2,3,4)
3=a*2+b*3+c*5
```

Function 0 calls function 1 with three parameters and returns 33. Parameters are
immutable. Loop updates are simultaneous. Scope rules, signed comparisons,
evaluation order and lazy branches are unchanged. No alternative operator spellings
or new core operations are introduced.

## Arithmetic contract

| Operation | v3 behavior |
|---|---|
| add, subtract, multiply | Two's-complement wrapping modulo 2^64 |
| signed divide | Truncate toward zero; divide by zero traps with E009 |
| MIN / -1 | MIN, without undefined behavior |
| comparisons | Signed i64 comparison, bool result |

Thus MAX+1 returns MIN, MIN-1 returns MAX, and MIN*-1 returns MIN. Debug/release and
O0/O2 agree. No overflow is implicitly undefined, and LLVM lowering makes no nsw/nuw
promises. Earlier profiles retain checked overflow and MIN/-1 traps. Selecting a
profile is therefore semantically significant even when source bytes are identical.
Validated HIR explicitly stores Checked/Wrapping arithmetic; backends do not inspect
source spelling. A module has one arithmetic mode, including every callee.

## Execution policy

Native v3 defaults to no fuel or call-depth instrumentation. These budgets are
tooling policy, not observable v3 arithmetic semantics. This removes counters and
gives LLVM freedom to simplify/vectorize loops. The native entry ABI and C wrapper
remain compatible with the existing benchmark harness.

```sh
cargo build --release -p nil --locked --offline
./target/release/nil --profile expr-v3 run examples/expr-v3/weighted.nil
# 33
./target/release/nil --profile expr-v3 build examples/expr-v3/sum.nil -o /tmp/nil-sum
/tmp/nil-sum
# 500500
./target/release/nil --profile expr-v3 --bounded run examples/expr-v3/wrapping.nil
# -9223372036854775808
./target/release/nil --profile expr-v3 llvm examples/expr-v3/sum.nil
```

`--bounded` (after the profile, before run/build/llvm) adds the existing 100,000-step
and 256-call limits without changing arithmetic. `--unbounded` disables accounting
explicitly, including for earlier checked profiles. Optimization level alone never
changes overflow or accounting policy. Source-size, parser-depth, type and HIR checks
are always enforced. Library Options expose Instrumentation and bounded limits.
The reference evaluator always requires explicit/default Limits and implements the
same wrapping arithmetic; a budget failure is an evaluation policy outcome.

Unbounded loops may never terminate; recursion can exhaust the host stack. Use
bounded execution for validation/repair of untrusted generated candidates. Wrapping
can also hide arithmetic mistakes, so source-token savings do not prove improved TCR.

## Performance evidence

[Recorded comparison](../../benchmarks/reports/2026-09-30/EXPR_V3.md) measures
v3, checked arithmetic without counters, old bounded v2 and ordinary/checked C++.
Both sum loops can become arithmetic formulas. Signed division still handles its
exceptional cases: a branch likelihood hint improves layout without changing results.
This is measured performance for the current integer core, not a universal C++ speed
or LLM-generation claim. Floats, aggregates, memory, plugins and final syntax selection
remain outside this experiment.

[Seeded fuzz testing](../FUZZING.md) exercises current capabilities and retains
reproducers for parser, checker, validator and native execution failures.
