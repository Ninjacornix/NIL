# LLVM benchmark rerun — 2026-09-30

Commit: `4117728e4d20383cfe39dc5237f5cbfd7531302b` (clean tree when measured).
Host: macOS-27.0-arm64-arm-64bit; Apple clang version 21.0.0 (clang-2100.1.1.101).
Python 3.12.11.

## Result

LLVM O2 was faster than Python and the reference evaluator in all eight cases.
All 192 oracle vectors agreed across Python, the evaluator and LLVM O0/O2.
No compiler or syntax changes were made for this rerun.

| Program | LLVM O2 ns/call | Python ns/call | Evaluator ns/call | Speedup over Python | Speedup over evaluator |
|---|---:|---:|---:|---:|---:|
| factorial | 13.3 | 226.7 | 1687.5 | 17.0× | 126.7× |
| fibonacci | 27.2 | 413.5 | 2881.7 | 15.2× | 105.7× |
| max | 2.4 | 45.1 | 177.6 | 18.5× | 73.1× |
| counted_sum | 135.8 | 2058.5 | 16031.2 | 15.2× | 118.1× |
| absolute | 2.6 | 43.6 | 211.5 | 16.7× | 80.7× |
| clamp | 2.6 | 49.7 | 266.8 | 19.0× | 101.9× |
| gcd | 6.4 | 121.9 | 736.6 | 18.9× | 114.4× |
| nested_sum | 106.0 | 1271.2 | 12461.5 | 12.0× | 117.6× |

## Measurement limits

Per-program backend builds took 90.5–103.1 ms (median 94.0 ms);
these are single build samples, not latency distributions. Runtime medians use nine
samples of 100,000 invocations each, with 1,000 warmups. Measurement order rotates
by case. Native execution retains checked arithmetic and fuel/depth limits.
Compilation, process launch, argument parsing and output formatting are excluded
from runtime timings. Tiny operations include call/measurement overhead.

These local microbenchmarks do not establish general performance superiority.
`nil run` recompiles every invocation; use `nil build` once for repeated execution.
Token counts and LLM success/repair rates are separate from execution speed.

## Reproduce

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/native.py --iterations 100000 --repeats 9 --output /tmp/native-llvm-rerun.json
```

[Raw results, O0/O2 samples, build stages and source hashes](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/native-llvm-rerun.json).
[Initial benchmark methodology](NATIVE_LLVM.md).
