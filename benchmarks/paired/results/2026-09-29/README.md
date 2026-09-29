# Local `lines-v0` / `expr-v0` experiment — 2026-09-29

The experimental expression profile lowered to the same HIR operations as the
line profile on all three programs. Both forms passed the same input/output
checks before timing. This run measures whole-file source tokens and steady
reference-interpreter execution; it does not measure model generation cost.

## Source tokens

| Tokenizer | Python | NIL `lines-v0` | NIL `expr-v0` | Expression change vs lines |
|---|---:|---:|---:|---:|
| `cl100k_base` | 87 | 146 | 65 | 55.5% fewer |
| Qwen2.5-Coder | 87 | 156 | 75 | 51.9% fewer |

`expr-v0` used fewer tokens than Python for each individual program under both
tokenizers. Across this corpus it used 25.3% fewer `cl100k_base` tokens and
13.8% fewer Qwen tokens than Python. The paired Python files include type
annotations. The result is specific to these three small arithmetic programs.

## Runtime

Median nanoseconds per call across three independent runs. Each run took the
median of seven timed batches of 500,000 calls after 10,000 warmup calls.
The Python column uses the measurements paired with `expr-v0`.

| Program | Python | NIL `lines-v0` | NIL `expr-v0` | Expression / Python |
|---|---:|---:|---:|---:|
| affine | 103.3 | 218.9 | 216.9 | 2.1× |
| squares | 166.6 | 487.5 | 485.5 | 2.9× |
| polynomial | 107.2 | 229.6 | 226.8 | 2.1× |

The two NIL profiles produce the same operations, and these runtimes are
effectively the same within machine noise. NIL's reference interpreter remains
slower than CPython on these samples. Runtime excludes parsing/compilation and
Python import; each timed call includes the result check. The programs are too
small to predict larger-workload or native-code speed.

## Reproduction and raw data

Machine: Intel Core i7-10700 (16 logical processors), Windows 11
10.0.26200, Python 3.12.10, Rust 1.98.1. Source revision:
`8d765f1558649c41bd6381f13b475bf3e73cdfa1`, clean working tree.
Tokenizers and assets are pinned in each JSON report. Run from the repository
root using the commands in [the benchmark README](../../README.md), adding
`--warmup 10000 --iterations 500000 --repeats 7 --format json` and the desired
manifest. Runs alternated between profiles to reduce order bias.

Raw records: [lines run 1](lines-1.json), [run 2](lines-2.json),
[run 3](lines-3.json); [expression run 1](expr-1.json),
[run 2](expr-2.json), [run 3](expr-3.json).
