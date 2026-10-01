# Compact profile experiment — 2026-09-30

Local measurements on macOS 27 arm64, CPython 3.12.11 and Rust 1.98.1.
Compiler source SHA-256 hashes, fixture hashes, dirty-worktree status, pinned
tokenizer revisions, all correctness vectors and raw timing samples are preserved
in [experiment.json](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/experiment.json). The source profiles are documented in
[EXPR_COMPACT.md](../../../docs/language/EXPR_COMPACT.md).

## Method

Compare 12 programs covering constants, parameters, all four arithmetic operations,
grouping, multi-argument functions, forward calls and nested calls. The same
algorithm is used in every version; no inlining or algebraic shortcuts.
Each has 28 expected-result vectors (three initial checks plus 25 seeded checks).
Rust tests additionally require identical HIR dumps for all three profiles.
Division vectors use nonnegative operands because Python `//` floors while NIL
division truncates; signed division and traps are checked separately in Rust.
Python has unbounded integers; equivalence is claimed within the tested i64 domain.

First remove declaration overhead (`expr-v1`), then replace parameter declarations
with arity (`expr-v2`). Both are opt-in and share the unchanged interpreter.
Compare against unannotated, compact `expr-v0`, single-line Python `def` functions
and equivalent Python lambdas. Count complete UTF-8 files, including final LF,
without framing or special tokens. The tokenizer is cl100k_base (not "clk100");
Qwen is Qwen2.5-Coder-1.5B-Instruct at the existing pinned asset revision.

Three rounds rotate profile order. Each timing has 5,000 warmup calls and nine
repeats of 50,000 calls. NIL/Python order alternates by case. Parsing, type checking,
Rust compilation and Python loading are excluded. Both timing loops check results.
Python lambdas receive one nine-repeat timing set after the profile rounds;
their timing comparison therefore has weaker order control.

## Token results

| Tokenizer | expr-v0 | expr-v1 | expr-v2 | Python def | Python lambda |
|---|---:|---:|---:|---:|---:|
| cl100k_base | 141 | 96 | 85 | 153 | 127 |
| qwen2.5_coder | 142 | 97 | 86 | 154 | 128 |

expr-v2 uses 39.7%/39.4% fewer tokens than expr-v0, 44.4%/44.2% fewer
than Python def, and 33.1%/32.8% fewer than Python lambda (cl100k/Qwen).
Every one of the 12 cases strictly beats expr-v0 and both Python baselines
under both tokenizers; the runner enforces this gate. expr-v1 also beats
expr-v0 and Python def on all cases. These are serialization/output-length
counts for checked programs, not measured model-generated tokens or TCR.

## Execution results

Nanoseconds per call: median of the three round medians, except Python lambda
(one set). Timing noise is expected; no speed threshold is enforced.

| Case | expr-v0 | expr-v2 | Python def | Python lambda |
|---|---:|---:|---:|---:|
| affine | 102.8 | 101.0 | 48.7 | 48.0 |
| squares | 220.3 | 232.7 | 83.4 | 87.2 |
| polynomial | 104.3 | 105.0 | 50.4 | 50.1 |
| constant | 31.5 | 32.0 | 28.1 | 27.4 |
| identity | 28.3 | 29.1 | 39.6 | 38.2 |
| add | 58.6 | 61.1 | 44.6 | 44.0 |
| difference | 60.0 | 64.2 | 43.4 | 43.3 |
| product | 63.8 | 59.6 | 43.2 | 43.5 |
| weighted | 114.8 | 117.0 | 58.3 | 57.2 |
| nested | 398.3 | 401.0 | 111.8 | 113.6 |
| grouped | 68.2 | 68.1 | 50.2 | 50.8 |
| quotient | 59.6 | 63.4 | 47.3 | 49.0 |

The NIL interpreter is slower than both Python forms in 11/12 cases; identity
is the exception. Shorter syntax does not reduce execution instructions:
HIR is identical, and differences across profiles are timing noise. No native
performance claim follows from these measurements.

## Reproduce

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/experiment.py --output /tmp/nil-experiment.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
./scripts/ci.sh
```

Keep expr-v0 as default. expr-v2 is the preferred candidate for a future
controlled generation study; model correctness and repair costs remain unknown.
This corpus is small, arithmetic-only, and the syntax was tuned on it. Test
unseen tasks and control flow when available before generalizing.
