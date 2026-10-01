# Typed expr-v4 corpus

25 paired programs exercise array sum, dot product, maximum, lower-bound binary
search, reverse, prefix sums (lengths 8/32/128/256), and a bool-returning helper.
NIL has immutable arrays with statically checked length and mandatory bounds traps.
C++ uses std::array by value and checked read/replacement helpers; Python replacement
copies a list. Callers observe no mutations. Search inputs are sorted and unique.
All measured numeric intermediates fit signed i64; Python arbitrary precision and
floor division are equivalent only within these specified domains. C++ uses -fwrapv
for arithmetic, and bounds errors abort rather than emitting NIL diagnostics.

```sh
python3 benchmarks/paired/typed/generate.py
# Install the pinned benchmark Python dependencies as described in ../README.md.
benchmarks/paired/.venv/bin/python benchmarks/paired/typed.py \
  --output /tmp/expr-v4-typed.json \
  --iterations 20000 --o0-iterations 2000 --python-iterations 2000 --repeats 7
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/paired/tests
```

The generator records independently computed expected outputs (sum/max/reversal,
zip products, list search and accumulate), separate from timed loop algorithms.
Every implementation checks all fixture outputs before timing. Native builds use
the same actual generated C driver and flat entry signature, separate translation
units and no LTO. Marshaling and result comparison are timed; parsing, startup and
compilation are excluded. Python iterations are recorded independently. Build-stage
figures are single observations, not stable distributions or performance gates.

Source counts include C++ required headers/helpers; function-only counts are also
reported to avoid attributing all boilerplate to the algorithm. Shared timing and
entry adapters are excluded for every language. Preserve reports with exact hashes,
tokenizer revisions, samples and environment. Signature screens are grammar
hypotheses, not model-generation experiments; repair cost/TCR remain unmeasured.

Fixed-size immutable arrays are a semantic experiment. In-place updates to fresh,
unaliased local results can be equivalent and faster in conventional languages;
compiler lowering should eliminate copies when safe. Compare that strategy before
claiming a win from value-copy baselines alone. This corpus does not model dynamic
allocation, arbitrary heap workloads, drivers or general-purpose language completeness.

For equivalent fresh-local mutation and isolated checker/payload measurements:

```sh
benchmarks/paired/.venv/bin/python benchmarks/paired/typed_local.py \
  --output /tmp/v4-local.json --iterations 20000 --python-iterations 2000 --repeats 7
benchmarks/paired/.venv/bin/python benchmarks/paired/typed_check.py --output /tmp/v4-validation.json
```

[Recorded results](../results/2026-09-30/EXPR_V4.md) include the slower large-array
updates, not just favorable copying baselines.
