# Milestone 5 — Token benchmark infrastructure

Status: **Core source measurement delivered; original unified schema/corpus plan partial**.

## Objective

Implement reproducible source measurement against exact tokenizers.

## Motivation

Replace spelling intuition with corpus-level evidence.

## Dependencies

M1 / NIL-014 for source measurement, already delivered. M4 / NIL-042 only for canonical-IR comparisons; not for a current-core M6 pilot.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-050 | Reuse benchmarks/paired and add a versioned attempt JSONL schema from benchmarks/README.md; add source hash, Unicode scalar/byte counts and tokenizer provenance. | NIL-014 |
| NIL-051 | Add pinned tokenizer adapters and equivalent NIL/Python/C/Rust baselines; include TypeScript or research IR only with verified semantics/provenance. | NIL-050 |
| NIL-052 | Add deterministic summaries, unavailable-count handling and corpus manifests; freeze held-out source corpus and publish reproducible results; add canonical-IR corpus only when serialization exists. | NIL-051; NIL-042 only for canonical IR |

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

## 2026-09-30 audit

NIL-050 source hashes, Unicode/byte/token counts and tokenizer provenance exist in
`benchmarks/paired/run.py`. NIL-051 has two pinned tokenizer adapters plus NIL,
Python and C++ baselines; separate C/Rust baselines remain unimplemented. NIL-052
has manifests, deterministic token summaries and recorded JSON reports. A dedicated
`tools/tokenbench` executable is optional packaging. Remaining work includes unified
per-attempt JSONL/accounting tests, held-out corpus freezing and canonical-IR
comparisons when required. Extend existing tools; do not redo delivered measurement.
See the [audit evidence](../MILESTONE_AUDIT.md).
