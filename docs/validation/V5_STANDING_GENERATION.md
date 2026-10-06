# Round 15 — standing generation evaluation

## NIL versus Python: no new model result

**The capable-model comparison is unmeasured, not NIL 0/48 versus Python 0/48.**
Isolation failed before any model request. The installed CLI could print its version
inside the sandbox and repository reads were denied, but its network-disabled
initialization exited with:

```text
Error processing settings: EPERM: operation not permitted, open '/tmp/claude-501'
Status: blocked-isolation attempts: 0 spend: 0.0
```

No initialization transcript was produced, so empty tools/MCP and disabled hook
activity could not be verified. No network-capable model invocation followed; no
held-out inference, calibration or post-DEV cost forecast ran. We did not test
whether claude-sonnet-5-5 is available to this subscription. No alternate model,
Ollama inference, simulated usage or extrapolated finding replaces it. The earlier
small-model NIL 0/24 versus Python 11/24 result remains the only live comparison.

## Forecasts versus actuals

Model pinned **claude-sonnet-5-5**, effort **high**. Claude Code **2.1.291**, Claude.ai
subscription/OAuth login, no API-key environment; bare cannot use that login.
Notional cap **USD 50**, USD 1 reserved/passed to the CLI for each next request.
No paid model request occurred; backend token usage and model solve rates are unknown.

| Arm | Forecast parse | Forecast solve | Actual |
|---|---:|---:|---|
| Python, no spec | 98% | 70% | Unmeasured |
| NIL exact spec | 70% | 25% | Unmeasured |
| NIL spec + fixed examples | 85% | 35% | Unmeasured |

Forecast NIL/Python output TCR 1.5–3×; full total TCR 10–40×. These remain judgment
forecasts, not validated estimates. Prefix-amortized cost is unknown: the two
backend input-accounting probes never ran. Do not replace it with tiktoken counts.
No paired model confidence interval is defined with no scored trials.

## Frozen design and reuse

[ADR 037](../adr/037.md) and benchmarks `generation/standing/experiment.json` /
`split.json` were committed before implementation and every attempted evaluation.
Main preregistration `791bab7`, benchmarks `d36741e`, both 2026-10-06
13:08:58 +02:00; implementation `d09e253` / `0be3c58` followed at 13:22:56 +02:00.
The stopped command used that implementation. Raw mock requests retain UTC times;
there are no live model request timestamps to invent. The offline probe's flags
and stderr are retained in `live-dev/isolation-probe.json`.

Held-out: **24 tasks**, 8 original byte/file, 12 external exercises, 4 algorithms;
**two replicates**, three total attempts, 512 output/request, 1,536 output/trial.
DEV: six disjoint reserved tasks; only `expand_tabs`, one replicate across arms,
is scheduled for pipeline confirmation. Maximum 432 held-out + 9 DEV + 2 input
accounting requests = **443** model CLI invocations, excluding network-blocked
initialization probes. No orchestrator infrastructure retries or substitutions.

Select supported, self-contained frozen contracts by SHA256(NIL-R15-v1/task-id)
within domain. Four fixed examples and any complete answer already in the mandated
spec are excluded. Canonical-JSON placeholder statements without explicit contracts
are listed as exclusions; no hidden fixtures are inserted to fill their gaps. This
cohort cannot establish general algorithmic coverage or training-set independence.
The exact published spec itself contains examples; spec-only means no *additional*
four-example block, not zero worked examples. Python receives the same task/file
contract without a language spec.

The adapter reuses the existing source extractor, repair history/real diagnostic
feedback, reference/native O0/O2/Python oracle, execution sandbox and corpus expected
values. Historical Ollama/application/few-shot files are unchanged. A separate
adapter is necessary because the frozen runner hard-codes four tasks, Ollama fields,
two profiles and a seed matrix. No eval service or new Python dependency was added;
existing Python 3.12/tiktoken environment supplies cl100k/o200k source screens.

## Failure taxonomy and confidence apparatus

Recorded per attempt: parse/type/compile-O0/compile-O2, correctness, diagnostic
code, trap, budget, wrong answer, infrastructure or isolation failure, source
feature associations, source bytes/chars/screens, backend fresh/cache input,
output, repair input/output, cost and wall time. Feature associations (loops,
callbacks, records, intrinsic names, modules, each) are descriptive, not causal.
**No real NIL failure-feature distribution exists this round.** The observed
failure is evaluator initialization/isolation, not a language parse failure.

Reports include per-task/arm vectors, first-pass solve rate, output/full-total TCR,
measured-prefix-once-per-trial input scenario, provider cache usage separately, and
2,000 paired task-cluster bootstrap intervals with undefined TCR draws counted.
No weighted score. Python native O0/O2/type rates are N/A. Missing usage stays
unknown; zero-solves TCR stays undefined. CLI budgeting may enforce after an API
call; any overshoot is retained rather than hidden. CLI internals are not a claim
of exactly one HTTP request per invocation.

## Offline evidence and gates

The **synthetic** full matrix uses reference answers solely as stub responses,
after an intentionally failing first attempt. It checks all 144 trial cells and
288 attempts against real reference/native/Python oracles, including real repair
feedback. All 48 stub trials per arm eventually solve, as expected from supplied
reference answers. This is harness evidence, **not model capability or token cost**.
Synthetic input/output/cost values are explicitly marked mock and cannot be passed
to the live cost forecaster. A smaller real-oracle DEV mock also passes.

```text
./scripts/ci.sh                         exit 0; 441 tests; zero warnings
./scripts/ci.sh release                 exit 0; 441 tests; zero warnings
standing harness unittest suite        exit 0; 15 tests
existing application unittest suite    exit 0; 6 tests
existing few-shot unittest suite       exit 0; 3 tests
--mock --phase held-out                 exit 0; 144 trials; 288 attempts; SYNTHETIC
--phase dev                            exit 2; blocked-isolation; zero model requests
corpus verify                          exit 0; 2815 checks
corpus verify --group original          exit 0; exactly 610 checks
git diff 0cf1da2 -- crates/ cli/ tools/ std/     empty
benchmarks diff 9830ffb -- corpora/ generation/application/  empty
```

Compiler, runtime, profiles, std, Rust dependencies, tasks, oracles, goldens and
baseline sources are unchanged. Main branch `feat/gen-eval` starts from
`origin/master` 0cf1da2 and has no upstream; benchmarks `exp/gen-eval` starts from
9830ffb. All commits local, nothing pushed. Pre-existing OVERSEER_HANDOFF.md is
untouched and untracked on this baseline.

## Reproduction and what is needed

```sh
# Offline tests and complete real-oracle synthetic matrix:
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/standing/tests -v
./scripts/gen-eval.sh --mock --phase dev --output /tmp/nil-gen/mock-dev
./scripts/gen-eval.sh --mock --phase held-out --output /tmp/nil-gen/mock-matrix

# Only after a CLI/auth configuration passes the same isolation proof:
./scripts/gen-eval.sh --phase dev --output /tmp/nil-gen/dev
benchmarks/paired/.venv/bin/python benchmarks/generation/standing/forecast.py --dev /tmp/nil-gen/dev
# Review and locally commit generation/standing/cost-forecast.json in benchmarks.
./scripts/gen-eval.sh --phase held-out --prior /tmp/nil-gen/dev --output /tmp/nil-gen/held-out

# Unchanged corpus verification:
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-gen/corpus.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-gen/original.json
```

A supported isolated CLI startup is needed first: audit its global temporary-state
dependency or provide a configuration/version that avoids it. API-key auth enables
bare, but **has not been proved to fix this EPERM**. Do not disable the sandbox or
allow arbitrary shared state to obtain a number. After successful DEV, the committed
actual-usage forecast must precede held-out. No forecast is manufactured now.
Summaries publish under benchmarks/reports/<UTC date>; raw transcripts/candidates
remain external. To test a later compiler/spec version, preregister its snapshot
before inference and preserve tasks/oracles/budgets; hash mismatches prevent silent
mid-experiment changes.

**No language or spec change is justified by this blocked run.** The next needed
work is evaluator isolation, not syntax tuning, training or performance work.

Raw evidence archive (credential-shaped strings checked; no executables):
`/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-standing-generation-2026-10-06.tar.gz`

SHA-256: `86ae8da7a74850b947bae2da774c24643336e66e9067c358012ede56c065af6d`.
