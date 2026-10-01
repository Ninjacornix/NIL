# expr-v3: compact source, C++-class native execution

## Outcome

The opt-in expr-v3 profile meets the current integer-core experiment target:
all eight measured programs run within 10% of ordinary Clang O2 C++ on both the
original workloads and larger inputs. V3 uses exactly the v2 grammar: 122 source
tokens across the suite under both pinned tokenizers. The actual compiled C++
source functions total 306 cl100k_base tokens and 314 Qwen tokens (60.1% and
61.1% fewer for NIL). This is raw-source efficiency, not measured LLM success/TCR.

Neither language is universally faster. Tiny timings include call/measurement
overhead, and the input corpus is small. The unchanged default remains expr-v0;
v3 must be selected explicitly. Its arithmetic is deterministic wrapping i64,
and native instruction/depth instrumentation is optional. This is a semantics
experiment with documented tradeoffs, not equivalent overflow behavior to C++.

## Method

Host: macOS-27.0-arm64-arm-64bit. Compiler: Apple clang version 21.0.0 (clang-2100.1.1.101).
Both languages use O2 with no LTO or CPU-specific flags. Both execute through
the identical C driver and entry ABI, separately compiled from the program.
Nine samples of one million invocations, each with 1,000 warmups; measurement
order rotates each repeat. Compilation/process startup/argv/output are excluded.
C++ fixtures are compiled from cpp/sources; generated ABI wrappers/drivers are
excluded from both source counts. C++ signed overflow is undefined; timed inputs
fit i64. V3 overflow wraps; tests cover its behavior outside C++ valid domains.

All 192 original oracle vectors agree across NIL and both C++ baselines in each
regular experiment. The stress run validates 194 vectors including its two new
timed inputs. The checked C++ baseline preserves v2 traps and fuel/depth rules.

## Regular workloads

| Program | V3 ns/call | C++ ns/call | V3 / C++ | V2 bounded ns/call | V2 without counters ns/call |
|---|---:|---:|---:|---:|---:|---:|
| factorial | 2.15 | 2.13 | 1.009 | 13.96 | 5.14 |
| fibonacci | 6.92 | 7.01 | 0.988 | 28.30 | 4.87 |
| max | 0.86 | 0.86 | 0.995 | 2.61 | 0.91 |
| counted_sum | 0.87 | 0.90 | 0.963 | 136.57 | 26.18 |
| absolute | 0.93 | 0.90 | 1.030 | 2.49 | 0.89 |
| clamp | 0.88 | 0.93 | 0.952 | 2.83 | 0.91 |
| gcd | 2.65 | 2.66 | 0.994 | 6.77 | 3.67 |
| nested_sum | 0.86 | 0.90 | 0.958 | 108.40 | 23.51 |

A ratio below 1 favors NIL. Small differences around 1 are not meaningful wins.
V2 columns come from separate paired runs; consult raw data for their C++ timings.
Removing counters helps but checked arithmetic still blocks formula replacement
in the sum loops. V3 allows LLVM to eliminate both loops while preserving wrapping
behavior. No undefined-overflow nsw/nuw promises are emitted.

## Larger workloads

| Program | Inputs | V3 ns/call | C++ ns/call | V3 / C++ |
|---|---|---:|---:|---:|
| factorial | [20] | 5.12 | 5.03 | 1.017 |
| fibonacci | [92] | 29.27 | 35.19 | 0.832 |
| max | [-9223372036854775808, 9223372036854775807] | 0.90 | 0.91 | 0.987 |
| counted_sum | [1000] | 0.86 | 0.87 | 0.990 |
| absolute | [-9223372036854775807] | 0.88 | 0.88 | 1.000 |
| clamp | [5, 0, 10] | 0.88 | 0.89 | 0.992 |
| gcd | [1836311903, 1134903170] | 73.97 | 73.85 | 1.002 |
| nested_sum | [40] | 0.86 | 0.87 | 0.984 |

## Lowering discovery and correctness

An initial v3 GCD run was approximately 16% slower. Assembly inspection showed
the rare -1-divisor path disrupting hot-loop layout. A nonsemantic branch
likelihood hint fixed layout; it does not remove the defined MIN/-1 case or zero
division check. Final boundary tests cover both optimization levels.

113 Rust tests pass in debug/release, including parser/canonical spelling, explicit
HIR arithmetic mode, native/reference differential signed grids, division zero and
MIN/-1, lazy branches, calls/recursion, bool/parallel loop state, all eight algorithms
at O0/O2, exact optional budgets and CLI profile/instrumentation handling. Sixteen
Python benchmark tests pass, including the C++ baseline fuel/trap tests.

Unbounded execution can run forever or exhaust the native stack. `--bounded`
restores limits while retaining wrapping arithmetic. Wrapping may hide generated
arithmetic mistakes; model-generation/repair experiments must quantify that tradeoff.

## Reproduce and raw data

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v3 --output /tmp/v3.json
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v3 --stress --output /tmp/v3-stress.json
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v2 --nil-instrumentation unbounded --output /tmp/v2-no-fuel.json
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --nil-profile expr-v2 --output /tmp/v2-bounded.json
./scripts/ci.sh
./scripts/ci.sh release
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

Raw samples, build stages, sizes, tokenizer revisions and exact source hashes:

- [V3 regular](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v3-cpp.json)
- [V3 larger inputs](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v3-cpp-stress.json)
- [V2 without counters](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v2-no-fuel-cpp.json)
- [V2 bounded](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v2-bounded-cpp.json)

Source hashes were checked against the final implementation. Results record the
parent commit and dirty tree; those hashes identify the measured compiler/scripts.
[V3 semantics](../../../docs/language/EXPR_V3.md) and
[ADR 013](../../../docs/adr/013.md) document the decision.
