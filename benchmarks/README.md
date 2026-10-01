# Benchmarks

Keep fixtures, oracles, runners and concise findings in Git. Raw samples, generated
programs, model responses, binaries and tokenizer caches live outside the repository.
Compiler correctness is checked with `./scripts/ci.sh`; benchmarks add focused
measurement checks without requiring an LLM.

## Contributor checks

From the repository root, using Python 3.12+ and the pinned Rust toolchain:

```sh
./scripts/bench.sh token --smoke
./scripts/bench.sh runtime --smoke
./scripts/bench.sh generation --mock
python3 -m unittest discover -s benchmarks/tests -v
```

Token smoke validates UTF-8 counts, independent counter plumbing, pinned-asset
rejection and paired comparison contracts with test counters. It does not measure
actual model tokenization. Runtime smoke checks compiled NIL, C++ and Python
against independent array/bool fixtures; it requires Clang (`NIL_CLANG` can select
it). Generation mock checks accounting, budgets, repairs, zero solves and audits
without Ollama, network access, tokenizer assets or a model. CI runs these checks;
it does not enforce noisy timing thresholds or perform inference.

## Optional full measurements

Install the locked tokenizer dependencies once. Full token runs may fetch the small
pinned tokenizer assets; they do not download model weights.

```sh
uv sync --project benchmarks/paired --locked
./scripts/bench.sh token --full
./scripts/bench.sh runtime --full
# Short development run, not a performance conclusion:
./scripts/bench.sh runtime --full --iterations 1000 --repeats 1
```

Runtime full measures both original and additional expr-v4 corpora against
fresh-local C++/Python. Repeat on the same idle host to assess performance.

Live model experiments are optional and require a separately running Ollama with
an installed model. Nothing starts a server or downloads weights automatically:

```sh
./scripts/bench.sh generation --ollama MODEL
```

That experiment tests adaptation to supplied NIL examples; it cannot establish
performance after training. The interrupted local pilot remains archived and is
not treated as completed evidence. See [generation controls](generation/README.md)
and the [measurement protocol](PROTOCOL.md).

## Outputs and sharing

Every command prints a fresh directory under
`~/.local/share/nil/benchmarks/runs/<timestamp>-<kind>-<id>/`. Set `NIL_BENCH_HOME`
or `--output /external/path` to change it. Existing directories and paths inside the
repository are rejected. The run manifest records settings, Git revision, dirty
source hashes, environment and status; full reports also identify tokenizer/model
versions. Share the run directory rather than committing it.

[Published reports](reports/README.md) retain concise findings. Historical raw data
links point to the immutable pre-cleanup commit; local archives retain byte-verified
copies. GitHub Actions uploads correctness artifacts for 14 days. Durable published
studies should attach raw runs to release assets or another maintained archive.

## Layout

- `bench.py` and `scripts/bench.sh`: public commands and artifact routing.
- `paired/`: existing token/runtime implementations, locked dependencies and shared
  arithmetic/control-flow/typed fixtures. Paths stay stable for reproducibility.
- `generation/`: optional model adapter, frozen tasks, accounting and mock/oracle tests.
- `tests/`: command routing, provenance and output-policy tests.
- `reports/`: versioned summaries; no raw result dumps.

Internal scripts remain available for specialized experiments. Use the public
commands for normal contributor checks and external output management.
