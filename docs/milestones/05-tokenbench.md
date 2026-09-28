# Milestone 5 — Token benchmark infrastructure

Status: **Planned; not implemented**.

## Objective

Implement reproducible source measurement against exact tokenizers.

## Motivation

Replace spelling intuition with corpus-level evidence.

## Dependencies

M1 for early measurement; M4 required for canonical-corpus comparisons; NIL-014, NIL-042.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-050 | Create tools/tokenbench and versioned JSONL schema from benchmarks/README.md; add source hash, Unicode scalar/byte counts and tokenizer provenance. | NIL-014 |
| NIL-051 | Add pinned tokenizer adapters and equivalent NIL/Python/C/Rust baselines; include TypeScript or research IR only with verified semantics/provenance. | NIL-050 |
| NIL-052 | Add deterministic summaries, unavailable-count handling and corpus manifests; freeze canonical corpus and publish reproducible sample results. | NIL-051, NIL-042 |

## Tests

Known tokenizer fixtures, Unicode byte/character separation, no implicit special tokens, schema validation, missing tokenizer behavior, paired program correctness and exact repeated counts.

## Benchmark requirements

Raw source-token count only; include whole-program/body-only modes and explicit baseline scaffolding. Pin assets/revisions and avoid network-dependent tests.

## Deliverables

Tokenbench CLI, adapters, paired corpus, manifests, raw machine-readable results and reproduction instructions.

## Acceptance criteria

At least two pinned tokenizers reproduce source counts on equivalent programs; unavailable counts are never silently estimated.

## Explicitly excluded work

Model generation, syntax winner selection, custom vocabulary, training and claims of TCR improvement.

## Risks and open questions

Provider message counts differ from raw token counts; credentials, rate limits and downloadable assets may require separate setup.
