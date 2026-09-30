# NIL LLVM vs C++ — 2026-09-30

Ordinary C++ is faster than NIL in all eight measured cases. Adding the same
arithmetic traps and HIR fuel/depth accounting to C++ brings performance close
to NIL. This does not establish a general language ranking.

## Controlled setup

- Host: macOS-27.0-arm64-arm-64bit.
- Compiler: Apple clang version 21.0.0 (clang-2100.1.1.101).
- Both NIL and C++ use Clang O2, host architecture, no LTO or architecture-specific flags.
- C++17 programs implement the same algorithms and i64 inputs, not handwritten formulas.
- Both use the exact generated NIL C timing driver and entry ABI, compiled separately
  from program objects so calls cannot be hoisted out of the timing loop.
- Nine samples of 1,000,000 calls each; 1,000 warmups per sample. Measurement order
  rotates each repeat. Reported values are medians; calls reset the context and check results.
- Ordinary C++ omits overflow traps and fuel/depth checks. Signed arithmetic outside
  its valid domain can have undefined behavior; all timed/correctness inputs fit.
- Checked C++ uses overflow builtins, checked division, instruction/region/return
  fuel charging and entry depth checks. It does not implement NIL source-span diagnostics.
- Compilation, process startup, parsing and output are excluded from execution timing.

## Execution results

| Program | NIL O2 ns/call | Ordinary C++ O2 ns/call | Checked C++ O2 ns/call | NIL / ordinary C++ | NIL / checked C++ |
|---|---:|---:|---:|---:|---:|
| factorial | 13.61 | 2.14 | 13.62 | 6.36× | 1.00× |
| fibonacci | 27.73 | 7.07 | 27.52 | 3.92× | 1.01× |
| max | 2.44 | 0.87 | 2.44 | 2.81× | 1.00× |
| counted_sum | 133.43 | 0.86 | 140.72 | 155.15× | 0.95× |
| absolute | 2.49 | 0.88 | 2.33 | 2.83× | 1.07× |
| clamp | 2.58 | 0.87 | 2.60 | 2.98× | 1.00× |
| gcd | 6.43 | 2.53 | 6.43 | 2.55× | 1.00× |
| nested_sum | 105.99 | 0.86 | 106.14 | 123.24× | 1.00× |

A ratio above 1 means NIL takes longer. Small differences versus checked C++
are not evidence of a meaningful win; these tiny kernels include call overhead.

Local assembly inspection confirmed that Clang removes the loops from ordinary
C++ counted_sum and nested_sum, producing the same short arithmetic sequence.
NIL preserves per-operation overflow/resource failures, so equivalent transformations
need proofs preserving those failures. Checked C++ retains comparable costs. The
large loop ratios therefore include both safety costs and lost optimization freedom,
not simply a constant overhead per check.

## Correctness and validation

All 192 checked-in oracle vectors agree for NIL, ordinary C++ and checked C++
(576 executable comparisons), also checked against the Python mathematical oracle.
The 16 Python benchmark tests pass, including new tests for checked C++ fuel
boundaries across all eight algorithms, depth restoration/exhaustion, and signed
overflow in factorial, Fibonacci, absolute value and division.

## Build costs

Single samples, not latency distributions. The C++ compile column includes parsing
and optimization; NIL backend time excludes its frontend, recorded separately in JSON.

| Program | NIL backend ms | Ordinary C++ build ms | Checked C++ build ms |
|---|---:|---:|---:|
| factorial | 579.0 | 148.7 | 112.8 |
| fibonacci | 99.9 | 111.5 | 115.0 |
| max | 96.9 | 109.6 | 110.4 |
| counted_sum | 95.4 | 111.9 | 111.0 |
| absolute | 95.5 | 111.7 | 108.7 |
| clamp | 100.1 | 110.3 | 111.4 |
| gcd | 99.1 | 108.7 | 113.9 |
| nested_sum | 102.4 | 110.4 | 117.4 |

C++ build totals include its program object, shared C wrapper and link. These are
small programs, not large-project compile benchmarks. Binary sizes, full runtime
samples, build stages, source hashes and compiler metadata are in
[native-cpp.json](native-cpp.json).

## Reproduce

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --output /tmp/native-cpp.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

The benchmark needs the repository’s Rust/Python environments and host Clang.
C++ sources: [program.cpp](../../cpp/program.cpp). The driver template is extracted
from nil-llvm runtime.rs; changes to that template should rerun the adapter tests.
No NIL compiler changes were made. Results were measured with parent commit
`4117728e4d20383cfe39dc5237f5cbfd7531302b` and new benchmark files in the working tree;
explicit source hashes identify the measured implementation.
