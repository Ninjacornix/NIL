# Milestone 7 — Syntax experiments

Status: **Partial prototype**. `expr-v0` is the compiler API and CLI default, and is paired against `lines-v0`. Model generation and trajectory ablations remain planned.

## Objective

Compare replaceable frontends while keeping semantic operations fixed.

## Motivation

Resolve the prefix-tree versus implicit-result research disagreement empirically.

## Dependencies

M4 and M6; NIL-042, NIL-062.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-070 | Implement prefix-tree alternative against existing HIR; describe numeric opcode/compact-ID/structured candidates as separate versioned profiles. | NIL-042, NIL-062 |
| NIL-071 | Add cross-frontend equivalence corpus and constrained-generation grammar experiment without changing semantics. | NIL-070 |
| NIL-072 | Run paired tokenizer and trajectory ablations; select or retain profiles using predeclared correctness and TCR criteria. | NIL-071 |

## Tests

Every profile lowers equivalent fixtures to the same canonical HIR; rejected syntax does not leak across profiles; diagnostics map to correct source spans.

## Benchmark requirements

Full-corpus tokens, parse/type/correctness rates, repairs and TCR; account for grammar and profile documentation costs.

## Deliverables

Independent frontend experiments, pinned profile definitions, equivalence tests and evidence-backed ADR.

## Acceptance criteria

At least line and prefix forms compared under the same oracle; selection justified by total model work and success, not character count.

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
