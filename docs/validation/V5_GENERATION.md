# expr-v5 generation experiment: command evidence

The compiler/runtime/profiles are unchanged from `7455d34`. The benchmark suite
adds application adapters around the existing provider transport, summaries and
paired task bootstrap. The frozen tasks and prompts have identical hashes before
and after the interpreter-access correction.

```text
./scripts/ci.sh
exit 0; 208 tests; zero warnings

./scripts/bench.sh generation --mock --output /tmp/nil-v5-existing-mock
exit 0; 7 offline accounting/audit tests; no inference

benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/tests -v
exit 0; 9 tests, including original reference/native numerical fixtures

benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/application/tests -v
exit 0; 6 tests with the pinned Python 3.12 interpreter

benchmarks/paired/.venv/bin/python benchmarks/generation/application/run.py --mock --output /tmp/nil-v5-generation-stub-final312
exit 0; 24 trials, 54 attempts, 18 synthetic solves

benchmarks/paired/.venv/bin/python benchmarks/generation/application/run.py --report-only --output /tmp/nil-v5-generation-stub-final312
exit 0; audit passed; STUB ONLY
```

Stub totals are deliberately synthetic: for each language, 288 output / 558 input /
168 repair-output units across 12 trials; nine solves and three budget-exhausted
failures. They test arithmetic, not model token efficiency. Real model usage is
kept in a separate directory and must never be pooled with this data.

Application tests cover all four contracts with handwritten solutions in both
languages, all frozen byte inputs, evaluator/native O0/O2 agreement, wrong bytes
with correct lengths, parse/type failures, extra stdout, input mutation, denied
network/other-fixture/user-data access and usage-tamper rejection. Every inference
run repeats the fixture preflight using its actual pinned interpreter before any
model request. No fixtures or expected bytes are sent to the models.

The stopped numerical pilot audit exits nonzero with `incomplete experiment`:
119 of 144 cells. The first application inference run was stopped/excluded because
a sandbox loader error invalidated Python results (14 completed attempts). The
corrected run starts afresh with unchanged tasks/prompts/budgets. No compiler bug
was discovered or fixed; instrumentation failures are explicitly documented in
[the audit](../experiments/V5_GENERATION_AUDIT.md).

The final measured commands, per-task and aggregate totals, repeat variation,
confidence intervals, extraction limitations and raw archive checksum are in
[the report](../../benchmarks/reports/2026-10-03/V5_GENERATION.md).

```text
benchmarks/paired/.venv/bin/python benchmarks/generation/application/run.py --output /tmp/nil-v5-generation-measured
exit 0; mandatory preflight passed; 48 trials / 122 attempts; REAL MODEL USAGE

benchmarks/paired/.venv/bin/python benchmarks/generation/application/run.py --report-only --output /tmp/nil-v5-generation-measured
exit 0; complete-cell/source/usage audit passed

benchmarks/paired/.venv/bin/python /tmp/nil-v5-fence-sensitivity.py
exit 0; 18 complete recorded blocks / 0 correct after label removal; no inference
```

Every valid-run provider usage count is present. V5 solves 0/24; Python 11/24;
no repairs succeed. Totals include all failed requests; source-byte counts are
not substituted for model counts. The temporary owned Ollama server/runner were
stopped. All commits are local; no push/upload.

Raw measured/excluded/stub artifacts and logs are archived separately inside
`~/.local/share/nil/benchmarks/archive/v5-generation-2026-10-03.zip`, SHA-256
`af6cac6da4f2e5740f14681bd2badae3c459fb232703c3695d11b98fdb856b6d`.
