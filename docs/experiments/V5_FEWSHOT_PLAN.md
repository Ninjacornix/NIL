# Fixed few-shot exposure experiment

This rule is committed before any generation request. It extends the frozen
application experiment, not the compiler, corpus or correctness oracle.

The control is the exact original prompt, which already includes a generic
copy-file example. The treatment adds four complete verified NIL examples,
including their task statements but no test inputs or expected answers. The
specification stays unchanged: deleting it would change a second variable.

Select from original supported corpus tasks with `adaptation_overlap=false`,
using four fixed categories in order: byte transform, numeric buffers, line
processing, formatting. Exclude `ascii_lower` (near-solution of evaluated
`ascii_upper`) and `copy_file` (already in control). Require every intrinsic to
occur in the frozen reference. Choose the lexicographically first remaining
ID per category: **invert_bytes, histogram_digits, count_newlines, hex_encode**.
No per-evaluation-task choice, failure-driven reselection or prompt tuning.
The rule and exact source hashes are in the benchmark experiment definition.
These examples cover updates, positional scope, helpers, lazy comparisons,
buffer state and output accumulation without introducing new library operations.

Use the same pinned Gemma 3 4B/Qwen 2.5 7B digests, four application tasks,
seeds 17/29/43, temperature, context and repair budgets. For each model, rerun the
original NIL/Python control, then NIL with examples: 24 NIL control, 24 NIL
few-shot, 24 unchanged Python baseline trials. Serial order is a limitation;
this is a within-session comparison, not a historical-control comparison.
No inference smoke trials or automatic retries. Audit exact hashes and usage.

Primary signal: per-attempt parse, typecheck and native-compile pass rates and
semantic/execution failures, alongside final-trial failures and solves. Count
all provider-reported input/output tokens, including failed attempts, repairs
and repeated example text. Report output and total TCR only when solves exist.
Compare Python honestly using its rerun common baseline, not the older eleven
solves. Report each model, condition, task and seed; four task clusters cannot
establish a general result. A null result does not logically rule out all
familiarity/adaptation strategies. No few-shot result proves fine-tuning works.

Both exact local digests were found on 2026-10-05 using Ollama 0.9.0, Python
3.12.11. The original oracle, source extraction (including its fence-label
restriction) and repair feedback are reused unchanged. Selected examples must
pass reference/native O0/O2 preflight again. A stub dry run demonstrates
accounting and repairs but is never evidence about model efficiency.
