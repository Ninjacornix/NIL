# NIL generation and repair experiment

Status: optional tooling; the local pilot was interrupted at the user’s request.
It provides no completed model-efficiency result. Raw artifacts are archived outside
the repository. Use `./scripts/bench.sh generation --mock` from the repository root for offline checks.

Compare expr-v0/v1/v2 under the same checked-i64 semantics. This isolates source
representation from overflow policy; v3/v4 semantic changes are separate studies.
Generated programs are parsed as NIL, never executed as Python or shell commands.
Reference evaluation and native execution both enforce instruction/depth budgets.

## Frozen pilot (2026-10-01)

- Installed local models: Gemma 3 4B and Qwen 2.5 7B Instruct. Record full model
  digests, model/template metadata and Ollama version before requests.
- Twelve identical task contracts and frozen oracles; eight existing development
  algorithms and four additional tasks. Examples are not contamination-proof and
  the additional tasks are not a statistically independent external holdout.
- All three profiles, seeds 17 and 29: 144 trials. Profile order rotates; model
  runs are sequential to avoid loading contention. Temperature 0.2, context 4096.
- At most three attempts, 256 generated tokens per request, 640 cumulative output
  tokens per trial. All reported output counts toward the budget and TCR; no
  success-only filtering. Hidden vectors are never sent to the model.
- Correctness requires every frozen vector to agree in the bounded reference
  evaluator and compiled native executable. Native build failure or timeout fails.
- Few-shot countdown/helper examples, scope/loop rules and task are included in
  prompt costs. Initial smoke failures informed generic scope reminders; smoke
  results are excluded from the frozen pilot. Repairs
  contain compiler errors or generic semantic feedback, not expected answers.
- Primary metric: failure-inclusive output TCR; also input+output TCR, Solve@budget,
  first-pass correctness, solved-only TTCP, raw source counts and repair tokens.
  Missing provider usage remains null and invalidates exact TCR; it is never zero.
- Paired bootstrap resamples whole tasks, retaining paired seeds/profiles. A
  candidate needs an upper 95% TCR-ratio bound below 0.90 and lower 95% solve-rate
  difference above -0.05 before recommending it over v0. Report undefined
  bootstrap samples and all failures. Small pilots can be inconclusive.

Ollama `eval_count` measures generated output and `prompt_eval_count` measures
input; both are retained with raw responses. Separately exposed thinking text is
recorded without adding an invented token count. Source token counts use the two
existing pinned tokenizers, not an estimate of Gemma's vocabulary. See the official
[chat API](https://docs.ollama.com/api/chat). Native process wall time includes
startup and is not claimed as steady-state runtime; unavailable isolated backend
and runtime measurements remain null. Existing paired benchmarks measure runtime.

## Run

```sh
cargo build --release --locked --offline -p nil-compiler --example generation_check -p nil
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/tests -v
# Start Ollama separately if needed: ollama serve
benchmarks/paired/.venv/bin/python benchmarks/generation/experiment.py --output /external/path/nil-pilot
```

Use a fresh output directory. Optional `--models`, `--seeds` and `--task-limit`
support separately labeled smoke runs; never pool different budgets/configurations.
After the complete run, audit and measure accepted programs on an idle host:

```sh
benchmarks/paired/.venv/bin/python benchmarks/generation/report.py /external/path/nil-pilot
# Unload the experiment models before timing: ollama stop MODEL
benchmarks/paired/.venv/bin/python benchmarks/generation/measure_correct.py /external/path/nil-pilot
```

The follow-up uses the existing in-process native driver with bounded O2 execution,
1000 warmups and five samples of 10000 calls. It records isolated build stages,
binary size and native runtime for accepted programs; unsuccessful attempts remain
charged in TCR. The paired environment/tokenizer setup is in `../paired/README.md`.

Each run retains configuration/provenance, per-attempt requests/responses/source,
append-only attempt/trial JSONL, an incremental summary and final paired intervals.
Interruptions leave partial artifacts; incomplete cells cannot justify a winner.
No compiler grammar/default changes follow automatically from pilot results.
