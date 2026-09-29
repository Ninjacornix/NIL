# Compact expression experiments

Status: opt-in experiments on `exp/token-reduction`. `expr-v0` remains the default.
These profiles preserve the current i64 semantic core and use its checker, HIR
validator and reference interpreter. They add no runtime operations.

## Iteration 1: `expr-v1`

Remove function labels and header parentheses; preserve named parameters:

```text
=b(20,22)
x,y=x+y
```

Each nonempty line defines one function. Functions receive declaration-order
IDs: 0, 1, 2, …; calls spell these as `a`, `b`, `c`, …, `z`, `aa`, `ab`, ….
The CLI still selects functions by numeric ID. Blank lines do not consume IDs.
Parameter lists are comma-separated ASCII identifiers; duplicate names are errors.
No parameter or result annotations are permitted: every value is explicitly
fixed by this profile's schema to i64. Empty lists start directly with `=`.

## Iteration 2: `expr-v2`

Replace parameter declarations with arity and positional names:

```text
=b(20,22)
2=a+b
```

```ebnf
function = [ positive_arity ], "=", expression ;
```

Arity is a canonical decimal integer from 1 to 4096. Absence means zero;
`0=42` is rejected to preserve one zero-parameter spelling. Parameters use the
same lowercase sequence as function references: `a` is argument 0, `b` argument
1, and `aa` argument 26. A bare name accesses a parameter; a name followed by
`(` calls a function. References outside the declared arity are errors.

Both profiles retain expr-v0's infix precedence, left associativity, canonical
i64 literals, forward calls, source/depth limits, structured diagnostic byte
spans, overflow traps, truncating division, fuel and call-depth limits.
Uppercase function references and reference IDs exceeding u32 are rejected.
Function declarations cannot span multiple lines; comments are unsupported.

## Run

```sh
cargo run -p nil --offline -- --profile expr-v2 run benchmarks/paired/samples/squares.v2 0 3 4
# 25
cargo run -p nil --offline -- --profile expr-v1 check benchmarks/paired/samples/squares.v1
```

## Tradeoffs and evidence

Declaration order becomes semantic: reordering a function requires updating
calls. Arity declarations can make invalid argument references easier to generate.
Using the same alphabet for functions and parameters may affect model repair.
These are explicit experimental choices, not finalized architecture decisions.
Restoring additional types will require a typed signature mechanism; this grammar
is only for the existing single-type core.

The [recorded experiment](../../benchmarks/paired/results/2026-09-30/README.md)
measures both tokenizers and execution, including compact Python baselines.
Shorter token streams do not establish better generation success or TCR.
Do not adopt either profile as default without generation/repair evidence and
broader programs. No optimizer or backend changes accompany these experiments.

## M2 extension

Both profiles now support typed comparisons and lazy `?:`. expr-v1 uses
`loop(...)`; expr-v2 uses `@(...)` as its sole loop spelling. See
[control-flow semantics](CONTROL_FLOW.md). The earlier arithmetic-only benchmark
is historical; [M2 results](../../benchmarks/paired/results/2026-09-30/CONTROL_FLOW.md)
retain that corpus under the updated interpreter and add frontend measurements.
