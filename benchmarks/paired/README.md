# Paired NIL/Python benchmark

This small corpus answers two local questions for the current `lines-v0` syntax:
does NIL use fewer raw source tokens than an equivalent Python program, and does
the already-compiled NIL reference interpreter execute the function faster than
CPython? Both comparisons are reported per case. This does not measure model
generation cost, token-to-correct-program, native code, or production workload
throughput. The full research protocol remains in [../README.md](../README.md).

The programs cover straight-line i64 arithmetic and calls, the capabilities NIL
currently has. Each case has several input/output checks; the first is timed.
Python integers are unbounded, so these checks stay within the i64 range. The
measurements apply to these programs and inputs only.

## Run

Requires Python 3.12, [uv](https://docs.astral.sh/uv/), and the repository's
pinned Rust 1.98.1 toolchain. From the repository root:

```sh
uv sync --project benchmarks/paired --locked
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py --format json > paired-results.json
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

`uv.lock` pins Python dependencies. The first `tiktoken` encoding load may fetch
its version-checked `cl100k_base` asset; later runs use the local cache. Rust
builds use `--locked --offline`. The runner builds `paired_runtime` in release
mode, checks every expected result in both languages, then records seven repeats
of 10,000 calls after 1,000 warmup calls by default. Adjust with `--iterations`,
`--warmup`, and `--repeats`. A failed correctness check stops the run.

The tokenizer counts each complete UTF-8 source file using `cl100k_base` from
the pinned `tiktoken` package. It treats special-token spellings as ordinary
text and adds no chat framing. Sample files are pinned to LF line endings by
`.gitattributes` so checkout settings do not change their counts. The JSON
includes source hashes, bytes, characters, raw token counts, tokenizer version,
per-repeat nanoseconds per
call, median, inputs, environment, and NIL/Python ratios. A ratio below 1
means NIL used fewer tokens or was faster. No token estimate is substituted
if the tokenizer is unavailable.

Runtime timing excludes Rust compilation and NIL parsing/checking, as well as
Python import/compilation. Each timed call includes the NIL evaluator or the
Python function and the runner's result check. Calls run in separate processes
under the same host; order is NIL then Python for every case. Small programs
are sensitive to call, clock, CPU scheduling, and allocation overhead. Repeat
on an idle machine before interpreting differences. This is interpreter versus
interpreter, not a claim about future NIL native speed. The token comparison
also omits specification, prompt, and repair costs.

The `cases.json` manifest holds pairs and correctness checks. Add a new pair
with the same algorithm and test vectors in `samples/`, then run the unit tests
and full benchmark. Generated JSON reports are local artifacts and should be
recorded with machine details if published; no CI timing threshold is installed.
