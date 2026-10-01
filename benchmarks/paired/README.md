# Paired NIL/Python benchmark

Implementation details and historical reproduction commands. For contributor
checks, external artifact routing and current commands, use the
[benchmark guide](../README.md).

This small corpus answers two local questions for `lines-v0` and `expr-v0` syntax:
does NIL use fewer raw source tokens than an equivalent Python program under
either of two model tokenizers, and does the already-compiled NIL reference
interpreter execute the function faster than CPython? Both comparisons are
reported per case and per tokenizer. This does not measure model
generation cost, token-to-correct-program, native code, or production workload
throughput. The full research protocol remains in [../PROTOCOL.md](../PROTOCOL.md).

The programs cover straight-line i64 arithmetic and calls, the capabilities NIL
currently has. Each case has several input/output checks; the first is timed.
`cases.json` measures `lines-v0`; `cases-expr.json` uses the same Python programs
and checks with the `expr-v0` sources (now the default profile). The compact expression fixtures
lower to the same HIR operations as their line-form counterparts.
Python integers are unbounded, so these checks stay within the i64 range. The
measurements apply to these programs and inputs only.

## Run

Requires Python 3.12, [uv](https://docs.astral.sh/uv/), and the repository's
pinned Rust 1.98.1 toolchain. From the repository root:

```sh
uv sync --project benchmarks/paired --locked
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py --manifest benchmarks/paired/cases-expr.json
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py --format json > paired-results.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

`uv.lock` pins Python dependencies. The first `tiktoken` encoding load may fetch
its version-checked `cl100k_base` asset. The runner also downloads the published
Qwen2.5-Coder tokenizer from a fixed model revision into the external `NIL_BENCH_HOME/cache/` directory and verifies
its SHA-256; later runs reuse it. [Qwen's model](https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct)
is Apache-2.0 licensed. Rust
builds use `--locked --offline`. The runner builds `paired_runtime` in release
mode, checks every expected result in both languages, then records seven repeats
of 10,000 calls after 1,000 warmup calls by default. Adjust with `--iterations`,
`--warmup`, and `--repeats`. A failed correctness check stops the run.

Both tokenizers count each complete UTF-8 source file: `cl100k_base` via
`tiktoken`, and Qwen2.5-Coder via Hugging Face `tokenizers` with its published
`tokenizer.json`. Neither adds chat framing or special tokens. Sample files
are pinned to LF line endings by `.gitattributes` so checkout settings do not
change their counts. The JSON includes source hashes, bytes, characters, raw
token counts for each tokenizer, package and asset revisions, per-repeat
nanoseconds per call, medians, inputs, environment, and NIL/Python ratios. A
ratio below 1 means NIL used fewer tokens or was faster. Counts are reported
separately; no cross-tokenizer average is calculated. No token estimate is
substituted if an asset is unavailable.

Runtime timing excludes Rust compilation and NIL parsing/checking, as well as
Python import/compilation. Each timed call includes the NIL evaluator or the
Python function and the runner's result check. Calls run in separate processes
under the same host; order alternates NIL/Python and Python/NIL across cases and is recorded in JSON. Small programs
are sensitive to call, clock, CPU scheduling, and allocation overhead. Repeat
on an idle machine before interpreting differences. This is interpreter versus
interpreter, not a claim about future NIL native speed. The token comparison
also omits specification, prompt, and repair costs.

The `cases.json` manifest holds pairs and correctness checks. Add a new pair
with the same algorithm and test vectors in `samples/`, then run the unit tests
and full benchmark. Generated JSON reports are local artifacts and should be
recorded with machine details if published; no CI timing threshold is installed.
The [2026-09-29 local experiment](../reports/2026-09-29/README.md) includes raw
reports for both source profiles.

## Compact-profile experiment

`experiment.py` compares opt-in expr-v1 and expr-v2 against compact expr-v0,
Python def and Python lambda across 12 identical programs, both tokenizers, and
rotated repeated runtime measurements. See [results and reproduction](../reports/2026-09-30/README.md).
The candidate must use strictly fewer tokens per case than expr-v0 and both Python
forms under each tokenizer. It does not need to execute faster to pass the token
gate; execution samples are reported separately.

## Milestone 2 control flow

`control.py` benchmarks eight equivalent control-flow algorithms in all expression
profiles and compact Python with both tokenizers. It records frontend latency
separately, rotates repeated execution measurements, enforces per-case token
reduction, and retains the M1 comparison. See
[results and commands](../reports/2026-09-30/CONTROL_FLOW.md).

## LLVM native execution

`native.py` measures native O0/O2 build stages, size and in-process runtime against
the same Python/reference algorithms. `nil run` now defaults to LLVM; the earlier
`run.py` and `control.py` intentionally retain the interpreter oracle measurements.
See [native report](../reports/2026-09-30/NATIVE_LLVM.md).

## C++ comparison

`uv run --project benchmarks/paired --locked python benchmarks/paired/cpp.py --output /tmp/native-cpp.json`
compares the eight expr-v2 control-flow kernels with Clang O2 C++17. It includes
ordinary C++ and a checked baseline with NIL arithmetic/fuel/depth rules. All
versions share the native C driver and entry ABI, compiled separately without LTO.
Timing order rotates each repeat; oracle vectors are verified before timing.
See [measured results](../reports/2026-09-30/NATIVE_CPP.md). Run the Python tests above
to validate the C++ adapter and checked baseline (Clang is required).

For the new semantics experiment, pass `--nil-profile expr-v3`, or
`--nil-profile expr-v2 --nil-instrumentation unbounded` to isolate overflow checks
from instruction accounting. `--stress` times larger valid inputs. Ordinary C++
fixtures under cpp/sources are the actual compiled programs (generated ABI wrappers
are excluded from token counts); cpp/program.cpp supplies the checked v2 baseline.
Both pinned tokenizers measure whole NIL/C++ source files. See
[expr-v3 evidence](../reports/2026-09-30/EXPR_V3.md).

## Typed functions and arrays

[The typed corpus](typed/README.md) provides 25 array/bool programs plus equivalent
fresh-local mutation baselines. `typed.py`, `typed_local.py` and `typed_check.py`
reuse the pinned tokenizers and measure source counts, checker/payload cost, compiler
stages, O0/O2 runtime and binary size. See [actual expr-v4 results](../reports/2026-09-30/EXPR_V4.md).
The local baselines prevent value-copy overhead from creating misleading speed
claims. Model generation, repair cost and TCR remain unmeasured.
