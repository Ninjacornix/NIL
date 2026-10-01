# Milestone 7 — Syntax experiments

Status: **Partial: lines-v0 and expr-v0/v1/v2/v3, equivalence tests and source-token studies implemented; generation/repair study pending**.

## Objective

Compare replaceable frontends while keeping semantic operations fixed.

## Motivation

Resolve the prefix-tree versus implicit-result research disagreement empirically.

## Dependencies

Existing HIR suffices for frontend equivalence; M6 / NIL-062 is required for model/repair selection. M4 / NIL-042 only for serialized-IR comparisons.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-070 | Reuse existing expression/compact-ID profiles; implement prefix, numeric opcode or structured candidates only with a stated hypothesis. | Existing HIR; NIL-062 for evidence-led new experiments |
| NIL-071 | Retain delivered equivalence corpus; optionally add constrained generation under fixed semantic policy. | Existing profiles; model adapter for constraint experiment |
| NIL-072 | Run paired tokenizer and trajectory ablations; select or retain profiles using predeclared correctness and TCR criteria. | NIL-071 |

## Tests

Every profile lowers equivalent fixtures to the same canonical HIR; rejected syntax does not leak across profiles; diagnostics map to correct source spans.

## Benchmark requirements

Full-corpus tokens, parse/type/correctness rates, repairs and TCR; account for grammar and profile documentation costs.

## Deliverables

Independent frontend experiments, pinned profile definitions, equivalence tests and evidence-backed ADR.

## Acceptance criteria

At least two existing profiles compared under a held-out model/oracle protocol with matched semantics; selection justified by total model work and success, not character count. A prefix candidate is optional.

## Explicitly excluded work

Changing semantics while comparing syntax, permanent unmeasured aliases and tokenizer training.

## Risks and open questions

Compact IDs may remove useful model context; per-model winners may differ; no improvement is a valid result.


## Default profile update

After the 2026-09-29 paired report showed fewer source tokens on all three arithmetic
cases for two tokenizers with equal HIR/results, `expr-v0` became the default in
`compile`, `check`, `hir` and `run`. `lines-v0` remains selectable. This small study
does not establish fewer generation tokens or lower tokens-to-correct; extend the
paired tasks and then run model trajectory experiments before freezing syntax.

## 2026-09-30 audit

Frontend equivalence, malformed/profile-specific grammar rejection and original
byte spans are already tested. Arithmetic and control-flow studies use pinned
cl100k/Qwen tokenizers. NIL-072 source measurements exist; model trajectories and
syntax selection do not. V3 shares v2 grammar but changes overflow/accounting:
compare v0/v1/v2 under checked policy for syntax, and keep semantic-policy ablations
separate. No need to invent another spelling to count this workstream as progress.
See the [audit](../MILESTONE_AUDIT.md).


## 2026-10-01 trajectory comparison

The [local Ollama pilot](../../benchmarks/generation/README.md) now measures
existing v0/v1/v2 representations under identical semantics and frozen budgets.
Do not pool v3/v4 arithmetic changes into syntax effects. New grammar candidates
remain contingent on hypotheses from failure/repair evidence; the source-token
winner is not automatically the trajectory winner.
