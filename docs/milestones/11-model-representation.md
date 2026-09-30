# Milestone 11 — Model-specific representation

Status: **Deferred investigation; evidence and budget gated**.

## Objective

Investigate constraints, vocabulary and adaptation after verified benchmark evidence.

## Motivation

Separate representation gains from training/tokenizer changes.

## Dependencies

M6/M7 and stable semantic corpus; NIL-062, NIL-072.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-110 | Freeze corpus, licenses, splits, semantics and baseline model/tokenizer; specify budget and ablation protocol. | NIL-062, NIL-072 |
| NIL-111 | Evaluate grammar/type constraints first; then independently test semantic tokens or tokenizer adaptation and fine-tuning as authorized experiments. | NIL-110 |
| NIL-112 | Publish base/adapted × constrained/free comparisons including correctness, TCR, latency and training cost. | NIL-111 |

## Tests

All training targets compile and pass tests; tokenizer round-trip, no leakage across held-out tasks, reproducible usage accounting and model/asset hashes.

## Benchmark requirements

Token counts alone cannot pass this milestone; measure functional success, repair cost, TCR and adaptation compute.

## Deliverables

Versioned dataset, experimental configuration and reproducible ablation report; model artifacts only where authorized/licensed.

## Acceptance criteria

Any claimed improvement survives paired held-out correctness/TCR evaluation; null or negative results are retained.

## Explicitly excluded work

Training from scratch, custom tokens before evidence, permanent vocabulary per third-party operation and unbudgeted model runs.

## Risks and open questions

Compute cost, licensing, tokenizer regression and model compatibility require an explicit experiment budget.

## 2026-09-30 audit

Custom tokenizer/vocabulary/training are not prerequisites for a usable NIL
compiler. Grammar-constrained generation may be an earlier M7 experiment using
existing tokens. M11 adaptation still needs held-out M6/M7 evidence and a budget;
no model/tokenizer adaptation is implemented. See the [audit](../MILESTONE_AUDIT.md).
