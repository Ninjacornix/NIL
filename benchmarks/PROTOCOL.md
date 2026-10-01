# NIL measurement protocol (v0)

## Objective

Character compression ≠ tokenizer compression ≠ LLM efficiency. Freeze this protocol
before optimizing source spellings. Research sources are `docs/about/misc/`; their
reported percentages are not reproduced results.

For a trial, stop at the first candidate passing the frozen semantic oracle or a
common output-token/repair budget. **TTCP-out** is cumulative generated output tokens
through that first correct candidate. Unsolved TTCP is null/censored, never zero.
**TCR = sum(output tokens spent across all trials, including failures) / number of
solved trials**. With zero solves, TCR is undefined/infinite, serialized as null with
`solved_count: 0`. A trial is a task/model/representation/seed combination; repeated
solves count once per trial. Always report Solve@budget, first-pass correctness,
solved-only TTCP median, and paired confidence intervals alongside TCR.

Generated tokens include all reported model output, not just extracted source.
Record reasoning tokens separately when exposed, and state whether provider totals
include them; never double-count. Repair tokens are output on attempts after zero.
TTCP-total adds actual per-request input usage, including repeated context and
feedback; diagnostic-token counts are a breakdown, not an extra addition.

## Required raw attempt record (JSONL in M5/M6)

| Fields | Measurement |
|---|---|
| task_id, trial_id, attempt, seed | Stable identity; attempt zero is initial generation |
| representation, profile_version, vocabulary_track | Core / plugin / framework; never pool tracks |
| source_sha256, source_characters, source_bytes | Unicode scalar count and UTF-8 bytes, exact artifact retained |
| source_tokens, tokenizer_id, tokenizer_revision | Raw source with recorded special-token policy; null if unavailable |
| model_id, revision, sampling, prompt_hash, test_hash | Freeze prompts, compiler revision, dependency/plugin versions too |
| input_tokens, generated_tokens, reasoning_tokens | Actual usage plus counting provenance; unknown is null |
| repair_tokens, cumulative_output_tokens, repair_attempts | Per-attempt and cumulative values |
| parse_ok, typecheck_ok, compile_ok | Separate stages; skipped stages null, not false success |
| tests_passed, tests_total, semantic_ok | Require nonempty tests and successful execution; timeout is failure |
| frontend_ns, backend_ns, runtime_ns | Separate timed phases; unavailable backend is null |
| binary_bytes, peak_rss_bytes, wall_to_correct_ns | Binary size only for generated artifacts, not compiler binary |
| diagnostic_code, diagnostic_tokens, failure_reason | Retain source, output, errors and repair artifacts |

## Experimental controls and gates

Use paired NIL/Python/C/Rust tasks, optionally TypeScript and verified research IRs.
Same integer semantics, overflow domain, hidden oracle, algorithm requirements,
sampling settings, number of trials and budgets. Separate whole-program/body-only,
zero-shot/adapted, free/constrained, and core/plugin/framework tracks. Include spec
and schema prompt costs. Count exact pinned tokenizer assets; provider message-count
estimates are not raw-source counts. Hold out tasks; deduplicate training overlap.
Report all failures, bootstrap intervals by task, and success-versus-budget curves.

M1 establishes a correctness fixture (`examples/add.nil`) and a dependency-free
release benchmark (command: `cargo bench -p nil-compiler --bench pipeline`) for frontend
and already-compiled interpreter execution. The JSON includes source counts; token
counts, native backend latency and binary size stay null. Run on an idle machine,
record OS/CPU/toolchain/revision, warm up, retain repeated samples; no noisy CI timing
gate yet. Future backend timing excludes parsing/type checking; native runtime must
be distinguished from interpreter time. Rust workspace build time is a separate metric.

M5 builds tokenbench and paired baselines; M6 measures model trajectories. Select
models, sample counts, acceptable correctness margin and performance envelope before
those runs. Suggested research savings targets (e.g. 30%) are hypotheses. No syntax
winner or efficiency claim is accepted from character counts or a tiny M1 fixture.
