# LLVM native benchmark — 2026-09-30

LLVM O2 executes faster than Python and the reference HIR evaluator in all eight
measured cases. This is a local microbenchmark, not a general performance guarantee.
The unchanged expr-v2 fixtures total 122 tokens versus Python’s 197 under each of
the two pinned tokenizers (38.1% fewer). Token counts are source measurements;
LLM generation, repair tokens and TCR have not been measured.

## Method

Host: macOS-27.0-arm64-arm-64bit; Python 3.12.11.
Toolchain: Apple clang version 21.0.0 (clang-2100.1.1.101). Rust uses the repository-pinned toolchain.
Each implementation runs the same algorithm and arguments. All 192 oracle vectors
agree for Python, the evaluator and LLVM O0/O2. Each runtime sample uses 1,000
warmups and 10,000 invocations; tables use the median of seven samples. Measurement
order rotates by case. Native calls retain arithmetic traps, fuel/depth guards and
a fresh execution context. LLVM and C objects are compiled separately without LTO.
Timing excludes process launch, argv parsing, compilation and output formatting.

## Runtime

| Program | LLVM O0 ns/call | LLVM O2 ns/call | Python ns/call | Evaluator ns/call | Python / O2 |
|---|---:|---:|---:|---:|---:|
| factorial | 178.4 | 13.3 | 222.8 | 1631.3 | 16.8× |
| fibonacci | 393.1 | 27.0 | 404.6 | 2813.9 | 15.0× |
| max | 8.0 | 2.5 | 43.9 | 175.6 | 17.6× |
| counted_sum | 1965.8 | 130.5 | 2042.5 | 15753.5 | 15.7× |
| absolute | 13.3 | 2.2 | 42.8 | 209.9 | 19.5× |
| clamp | 13.3 | 3.0 | 49.9 | 259.4 | 16.6× |
| gcd | 64.3 | 6.3 | 118.6 | 722.2 | 18.8× |
| nested_sum | 1571.7 | 103.5 | 1165.0 | 11810.0 | 11.3× |

## Compilation and artifact size

These are single build measurements, not repeated latency distributions. The
frontend column includes parse, type check and HIR validation. Backend total also
includes staging/I/O overhead; it is not the sum of runtime measurements. Clang
process launches and linking dominate these small programs. `nil run` rebuilds
each invocation; standalone executables avoid that cost.

| Program (O2) | Frontend µs | IR lowering µs | LLVM codegen ms | C wrapper ms | Link ms | Backend total ms | Binary bytes |
|---|---:|---:|---:|---:|---:|---:|---:|
| factorial | 13.4 | 25.4 | 23.8 | 34.9 | 36.1 | 95.2 | 34136 |
| fibonacci | 13.9 | 31.2 | 25.4 | 34.8 | 32.0 | 92.6 | 34136 |
| max | 10.2 | 21.1 | 21.7 | 35.1 | 31.8 | 88.9 | 34136 |
| counted_sum | 12.1 | 26.5 | 23.8 | 35.0 | 31.5 | 90.7 | 34136 |
| absolute | 9.8 | 23.0 | 21.5 | 34.8 | 32.0 | 88.7 | 34136 |
| clamp | 11.1 | 24.0 | 21.8 | 35.2 | 32.8 | 90.1 | 34136 |
| gcd | 12.1 | 29.7 | 24.3 | 35.5 | 31.9 | 92.1 | 34136 |
| nested_sum | 17.1 | 34.6 | 27.1 | 35.6 | 32.1 | 95.2 | 34136 |

The shared wrapper contributes substantially to binary size. These macOS executables
use the system C runtime; sizes do not include system libraries. O0 timings, build
stages, raw runtime samples, tokenizer revisions, source hashes and environment
metadata are in [native-llvm.json](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/native-llvm.json). The recorded revision is the
parent commit with a dirty tree; explicit Rust source hashes identify the measured
implementation. Historical M2 interpreter timings remain separate.

## Reproduce

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/native.py --output /tmp/native-llvm.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

Clang and the pinned Rust/Python environments are required. Repeat on an idle host
before drawing broader conclusions. Native LLVM changes execution, not the NIL
syntax or token efficiency; no conclusion about LLM success rates follows here.
