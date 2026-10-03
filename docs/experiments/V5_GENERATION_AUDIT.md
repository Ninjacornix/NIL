# Application generation audit (2026-10-03)

## Existing apparatus and stopped pilot

`benchmarks/generation/experiment.py` generates expr-v0/v1/v2 numerical
programs from 12 fixed contracts. It uses Ollama `/api/chat`, seeds 17/29,
temperature 0.2, context 4096, three attempts, 256 output tokens per request and
640 per trial. Requests/responses/source and attempt/trial JSONL are retained.
Provider `eval_count`/`prompt_eval_count` give output/input usage. Failed attempts
and unsolved trials contribute to TCR; unavailable usage stays null. First-pass,
Solve@budget and task-cluster paired intervals accompany TCR.

`generation_check.rs` is a bounded i64-only reference adapter; native candidates
also run bounded. `report.py` audits completeness, provenance, usage and accepted
vectors. `measure_correct.py` measures only accepted numerical programs after
inference; it cannot establish generation efficiency alone. The paired suite
counts handwritten source using pinned tokenizers and compares execution. None
of these studies measured v5 application repair efficiency or a mainstream
model-generated baseline.

The archived 2026-10-01 smoke contains six trials / 18 attempts / zero solves.
The stopped pilot contains **119 of 144 planned trials**, 351 attempts and six
solves: Gemma completed 72 trials; Qwen completed 47 of 72. Failure attempts were
210 type, 95 parse and 40 semantic. These are partial descriptive counts, not a
complete comparison. Smoke feedback influenced generic scope reminders; it is
excluded from the frozen pilot. Familiar numerical examples are not an
independent contamination-proof holdout. No winner, v5 result or mainstream
comparison follows, even if a subset can be summarized.

Artifacts were inspected directly under
`~/.local/share/nil/benchmarks/archive/2026-10-01-repository-cleanup/benchmarks/generation/results/2026-10-01/`.
The existing complete-cell audit rejects the pilot as incomplete.

## Access available here

Initial `curl --max-time 3 http://127.0.0.1:11434/api/tags` exited 7:
Ollama was stopped. Both frozen model manifests and every referenced blob were
present with matching sizes. Manifest SHA-256 matches the archived model digest:

| Model | Digest | Quantization |
|---|---|---|
| gemma3:4b | a2af6cc3eb7fa8be8504abaf9b04e88f17a119ec3f04a3addf55f92841195f5a | Q4_K_M |
| qwen2.5:7b-instruct | 845dbda0ea48ed749caafd9e6037047aa19acfcfd82e704d7ca97d631a0b697e | Q4_K_M |

A temporary `OLLAMA_HOST=127.0.0.1:11434 ollama serve` exposed those exact tags;
`/api/version` returned **0.9.0**. No model was pulled, substituted or adapted.
This goal explicitly authorizes the new experiment; it does not resume or pool
the stopped pilot. The server is stopped after this run.

## What is reused and extended

The application adapter imports existing `post`, `summary`, `paired_interval`,
provenance and benchmark-helper utilities. It uses the same request/response
logging and failure-inclusive accounting. It adds frozen task/language prompts,
a two-byte-path entry adapter, in-memory reference Host, native O0/O2 file
verification, Python entry execution, and audits for those shapes.

These adapters are necessary: the original checker accepts only integer arguments
and scalar results, prohibits host I/O, and the original runner/audit hard-code
three NIL numerical profiles. A new independent metric/provider harness was not
built. Compiler, runtime, syntax and profiles remain unchanged. Runtime timing
and accepted-program optimization are deliberately absent from this study.

The executable experiment definition and commands are in
[the application guide](../../benchmarks/generation/application/README.md).
Final measured findings and confidence limits are recorded in
[the report](../../benchmarks/reports/2026-10-03/V5_GENERATION.md).

## Infrastructure correction before the valid run

The first live run was stopped after 14 completed attempts (eight Python) when
a correct generated Python program failed. The sandbox denied uv's Python 3.12
`libpython3.12.dylib` under the user home; an earlier preflight had used the system
Python. These attempts are retained as an excluded infrastructure run, not charged
to either language in the valid comparison. No tasks, prompts, seeds or budgets
were changed. The sandbox now admits only the exact pinned interpreter installation
within its user-data deny rule, and every real/stub run validates all eight
handwritten task/language fixtures with that interpreter before any model request.

The corrected definition is benchmark commit `eea0995`; compiler documentation
commit `91e3d60` contains no compiler changes. Preflight, all six application tests
and the 24-trial/54-attempt stub pass with Python 3.12. The stub is synthetic
accounting evidence only; it is excluded from all model-token findings.
