# M2 control-flow semantics

Implemented in expr-v0, expr-v1 and expr-v2. The default remains expr-v0;
lines-v0 remains the arithmetic-only compatibility frontend. All expression
function parameters/results remain i64. Bool is a distinct internal type for
conditions, conditional results and loop state; there is no integer truthiness.

## Comparisons and conditionals

Signed i64 comparisons are `<`, `<=`, `>`, `>=`, `==`, `!=`; each produces bool.
The literals `true` and `false` also produce bool. Bool operands cannot be used
in arithmetic or integer comparisons. Conditions must be bool.

```text
2=a>b?a:b
```

This expr-v2 function returns the larger argument. A conditional evaluates its
condition, then exactly one branch. Both branches are statically checked and must
produce the same type. Inactive branches cannot overflow, divide by zero or call
functions at runtime. Unknown functions in inactive branches are still errors.

Precedence from highest to lowest: calls/parentheses, `* /`, `+ -`, comparisons,
`?:`. Arithmetic and comparisons associate left-to-right; chained comparisons
are rejected by typing because the first result is bool. Conditionals associate
to the right: `p?x:q?y:z` means `p?x:(q?y:z)`. Parentheses permit nested conditions.
There are no logical operators or implicit conversions in M2.

## One loop construct

expr-v2 uses `@(initials;condition;updates;finish)`. expr-v0/v1 use
`loop(initials;condition;updates;finish)`. Each profile accepts only its spelling.
Initializers and updates are comma-separated expression lists. The other two
sections each contain exactly one expression. Zero-state loops use empty lists:
`@(;false;;42)`. No additional loop syntax, break or continue exists.

```text
1=@(1,a;b>1;a*b,b-1;a)
```

This computes factorial. Execution is precisely:

1. Evaluate initializers once, left to right, in the enclosing scope.
2. Bind state values to positional names `a`, `b`, …, `z`, `aa`, ….
3. Evaluate the bool condition against that state.
4. When true, evaluate updates left to right against the **old** state. Replace
   the entire state tuple simultaneously and repeat the condition.
5. When false, evaluate finish against the last state and yield its value.

Each update position must match its initializer's type; update count must match
state count. There is no widening or bool/int conversion. Condition/update/finish
regions see only state names, not outer function parameters or other region locals.
Pass needed outer values as initializers. Nested loops have their own state scope;
nested branches inherit the current scope. No state escapes except the yielded
finish value. Outer values remain immutable.

Fibonacci without computing an overflowing, unused F93:

```text
1=@(0,1,a;c>1;b,a+b,c-1;c==0?a:b)
```

## HIR and execution invariants

Functions and regions contain typed immutable instruction values. Branch regions
inherit available parent values; new region values cannot escape to a sibling or
parent. Each branch yields one value. Loop regions receive state-only operands;
condition yields one bool, body yields the next state, finish yields one value.
The independent HIR validator checks scopes, types, region result counts, calls
and annotations even for unreachable branches. Only validated HIR can execute.
HIR/AST region nesting is capped at 32; parser expression depth stays capped at
128. The evaluator uses an explicit stack, including calls inside regions.
Function-call depth counts functions, not regions.

Every executed instruction and function/region yield consumes one fuel step.
Empty condition/body regions therefore cannot create an unbounded zero-cost loop.
Inactive regions cost no steps. Defaults remain 100,000 steps and 256 function
frames. Checked i64 arithmetic, signed truncating division and E009 traps remain.
Structured E001/E004/E005/E006/E007/E008 diagnostics retain original byte spans.

## Acceptance domains

| Program | Input domain | Reference |
|---|---|---|
| factorial | 0..20 | product 1 through n; 0! = 1 |
| iterative Fibonacci | 0..92 | F0 = 0, F1 = 1 |
| max | all pairs of i64 | larger operand, ties unchanged |
| counted sum | 0..1000 under default fuel | n(n+1)/2 |

Factorial 21 and Fibonacci 93 trap on overflow. Larger counted sums may require
explicitly increased library execution limits. Negative factorial/Fibonacci inputs
are outside their example contracts; the examples do not enforce domain checks.

See [ADR 011](../adr/011.md), [examples](../../examples/README.md) and
[measured token/frontend/runtime results](../../benchmarks/reports/2026-09-30/CONTROL_FLOW.md).
Native codegen and standard LLVM optimization were added after M2 (see below).
Mutable locals, general aggregates and richer source signatures remain future work.

## Default execution backend

LLVM now compiles these semantics to native code. `nil run` uses compilation by
default; the reference interpreter is retained for tests/benchmarks. See
[native commands and limitations](NATIVE_LLVM.md). Earlier interpreter-only
measurements describe their historical implementation.
