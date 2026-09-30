# NIL sample programs — default `expr-v0`

The default profile now uses compact function expressions. All examples use the
i64 core with integer arithmetic, function calls, M2 comparisons, lazy branches
and structured loops. Arrays and external operations are not implemented. Function 0 is a no-argument
demonstration; function 1 accepts the problem inputs. Every `.nil` file has a
`.stdout` fixture, checked automatically by the CLI test suite.

Run from the repository root with Clang installed (`nil run` compiles through LLVM):

```sh
cargo run -p nil --offline -- run examples/leetcode_bank.nil
# 96 (20 days)
cargo run -p nil --offline -- run examples/leetcode_bank.nil 1 10
# 37 (function 1, argument 10)
```

| File | Function 1 arguments | Meaning | Default result |
|---|---|---|---:|
| `add.nil` | `a b` | a + b | 42 |
| `polynomial.nil` | `a b c x` | a*x² + b*x + c | 69 |
| `sum_of_squares.nil` | `a b` | a² + b² through helper calls | 25 |
| `count_odds.nil` | `low high` | Odd integers in the inclusive interval | 3 |
| `smallest_even_multiple.nil` | `n` | Smallest positive multiple of n and 2 | 10 |
| `leetcode_bank.nil` | `n` | Total deposits after n days | 96 |

## LeetCode problems supported by M1

These are original NIL solutions to the linked problems, tested against independent
Rust oracles. They are not submissions to or verdicts from LeetCode. Formulas cover
the full published domains; callers must honor constraints because these examples do not perform conditional input validation.

### 1523 — Count Odd Numbers in an Interval Range

[Problem and constraints](https://leetcode.com/problems/count-odd-numbers-in-an-interval-range/):
`0 <= low <= high <= 10^9`. Formula: `(high + 1) / 2 - low / 2`.

```sh
cargo run -p nil --offline -- run examples/count_odds.nil 1 3 7
# 3
cargo run -p nil --offline -- run examples/count_odds.nil 1 8 10
# 1
```

### 2413 — Smallest Even Multiple

[Problem and constraints](https://leetcode.com/problems/smallest-even-multiple/):
`1 <= n <= 150`. Compute parity as `n - 2*(n/2)`, then return
`n*(1 + parity)`. This avoids needing remainder, comparisons or branches.

```sh
cargo run -p nil --offline -- run examples/smallest_even_multiple.nil 1 6
# 6
```

### 1716 — Calculate Money in Leetcode Bank

[Problem and constraints](https://leetcode.com/problems/calculate-money-in-leetcode-bank/):
`1 <= n <= 1000`. Deposits start at 1 on the first Monday, increase each day, and
each following Monday starts one higher than the previous. Let `w=n/7` and `r=n-7*w`.
The total is `28*w + 7*w*(w-1)/2 + r*(r+1)/2 + w*r`. Function 1 evaluates this
closed form; all intermediates fit i64 in the problem domain.

```sh
cargo run -p nil --offline -- run examples/leetcode_bank.nil 1 10
# 37
```

## Tests

```sh
cargo test -p nil-compiler --test examples --locked --offline
cargo test -p nil --test cli --locked --offline
./scripts/ci.sh
./scripts/ci.sh release
```

Tests exhaust all 150 inputs for #2413 and all 1,000 inputs for #1716; compare #1523
against enumeration for every interval within 0..100 and test upper boundaries.
Generic samples use signed grids. Oracles enumerate/simulate or use Horner's rule,
independent of the NIL formulas. CI runs every default-profile example against its
checked-in result. `lines-v0` remains available explicitly with `--profile lines-v0`.

## Milestone 2 acceptance programs

| File | Function 1 inputs | Valid domain | Default result |
|---|---|---|---:|
| factorial.nil | n | 0..20 | 3628800 (n=10) |
| fibonacci.nil | n | 0..92 | 55 (n=10) |
| max.nil | x y | all i64 pairs | 42 |
| counted_sum.nil | n | 0..1000 under default fuel | 5050 (n=100) |

```sh
cargo run -p nil --offline -- run examples/fibonacci.nil 1 92
# 7540113804746346429
cargo run -p nil --offline -- --profile expr-v2 run benchmarks/paired/control-samples/factorial.v2.nil 0 10
# 3628800
```

The default examples use expr-v0. Equivalent expr-v0/v1/v2 and Python control-flow
fixtures live in benchmarks/paired/control-samples. See
[M2 semantics](../docs/language/CONTROL_FLOW.md) and
[benchmark results](../benchmarks/paired/results/2026-09-30/CONTROL_FLOW.md).

For repeated execution, build once with `nil build examples/fibonacci.nil --entry 1
-o /tmp/nil-fibonacci`, then run `/tmp/nil-fibonacci 92`. See
[native compilation](../docs/language/NATIVE_LLVM.md).
