# Milestone 2 control-flow benchmark

[Raw machine-readable report](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/control-flow.json) includes source/manifest hashes,
compiler Rust source hashes, tokenizer revisions, execution samples, frontend
samples, platform/toolchain metadata and a rerun of the M1 arithmetic corpus.

## Method

Eight equivalent algorithms, using compact unannotated Python functions and
expr-v0/v1/v2. Python indentation is one space and loop bodies are compact; the
comparison does not inflate Python with type hints, comments or scaffolding.
Compare the same algorithms, not Python library shortcuts (math.factorial, sum,
etc.). Those may be shorter and faster; these results do not establish a win
against every valid Python solution. Initializer/update order is identical.

There are 192 reference vectors, including zero/one/many iterations and signed
bounds. Expected factorial/GCD results use mathematical references, Fibonacci
vectors use fast doubling, and sums use n(n+1)/2. Rust additionally exhausts the
factorial 0..20, Fibonacci 0..92 and counted-sum 0..1000 domains and tests max at
i64 extrema. All source profiles must produce the same HIR.

Whole UTF-8 file token counts include final LF and exclude chat framing/special
 tokens. cl100k_base and Qwen2.5-Coder use the existing pinned tokenizer assets.
No LLM is invoked: these are raw checked-source/output-serialization counts,
not generation success, repair tokens or TCR.

Three rounds rotate source-profile order; NIL/Python timing order alternates by
case. Each runtime set has 100 warmups and seven repeats of 1,000 calls, using
each case's first correctness vector. Parsing and checking are excluded from
runtime timing. Complete frontend timings separately include parsing, lowering,
independent HIR validation and result destruction (100 compile warmups, seven
repeats of 1,000 compiles). The frontend is compared across NIL profiles, not
against Python compile time. No backend/native timing exists because no native
backend is implemented. Noise and allocation/call overhead remain significant.

## Tokens

| Case | expr-v0 | expr-v1 | expr-v2 | Python |
|---|---:|---:|---:|---:|
| factorial | 18 | 16 | 15 | 23 |
| fibonacci | 25 | 23 | 22 | 34 |
| max | 11 | 8 | 6 | 13 |
| counted_sum | 18 | 16 | 15 | 23 |
| absolute | 12 | 9 | 9 | 14 |
| clamp | 20 | 17 | 11 | 23 |
| gcd | 18 | 16 | 15 | 21 |
| nested_sum | 33 | 31 | 29 | 46 |
| **Total** | **155** | **136** | **122** | **197** |

Counts are identical across the two tokenizers for this corpus. expr-v2 uses
**38.1% fewer tokens than Python** and **21.3% fewer than expr-v0**, strictly fewer
per case under both tokenizers. The runner fails if this gate does not hold.
An empirical token-only spelling screen compares the loop keyword with @ under
both tokenizers: keyword spelling totals 127 tokens, @ totals 122. The screen
preserves both candidate strings and per-case counts in the raw report; keyword
spelling is rejected by the final expr-v2 parser. Each profile has one loop form.

## Frontend and execution

expr-v2 timings below are medians of the three round medians. Nanoseconds per
compile/call; execution inputs appear in parentheses. Runtime includes the
result check in each language. This is reference interpreter versus CPython.

| Program | Input | Frontend ns | NIL ns/call | Python ns/call | NIL/Python |
|---|---|---:|---:|---:|---:|
| factorial | [10] | 1612.1 | 1688.7 | 227.9 | 7.41 |
| fibonacci | [20] | 2348.8 | 2870.8 | 406.5 | 7.06 |
| max | [20, 42] | 921.1 | 260.3 | 43.2 | 6.02 |
| counted_sum | [100] | 1641.5 | 15808.3 | 2044.0 | 7.73 |
| absolute | [-5] | 1137.1 | 291.3 | 42.6 | 6.83 |
| clamp | [12, 0, 10] | 1406.8 | 370.7 | 49.2 | 7.53 |
| gcd | [1071, 462] | 1658.8 | 862.0 | 117.2 | 7.35 |
| nested_sum | [10] | 3152.6 | 11878.4 | 1174.6 | 10.11 |

NIL is slower than Python on all eight cases. The interpreter allocates region
frames and state vectors; source-token reduction does not remove those costs.
No runtime optimization or native-code claim is made. All three profiles lower
identically; differences between their execution timings are measurement noise.

## M1 retained comparison

The same 12 arithmetic examples were rerun with the updated interpreter, rather
 than reusing historical timings. Token totals remain 85/86 for expr-v2 and
153/154 for Python under cl100k/Qwen. Per-case runtime samples are in
`m1_reference` in the raw report. Historical comparison with Python lambdas
remains separately recorded in [README.md](README.md).

## Reproduce

```sh
uv run --project benchmarks/paired --locked python benchmarks/paired/control.py --output /tmp/control-flow.json
./scripts/ci.sh
./scripts/ci.sh release
uv run --project benchmarks/paired --locked python -m unittest discover -s benchmarks/paired/tests
```

Grammar, scope, domains and safety limits are specified in
[CONTROL_FLOW.md](../../../docs/language/CONTROL_FLOW.md). The eight fixtures
live in [control-samples](../../paired/control-samples). This corpus helped select the
loop marker; unseen tasks and model generation/repair tests remain future work.
expr-v0 stays default; expr-v2 is explicit and experimental.
