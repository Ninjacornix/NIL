# Milestone 9 — First semantic framework

Status: **Deferred until plugin and measurement gates; not implemented**.

## Objective

Test meaningful domain compression with one narrowly scoped extension.

## Motivation

Measure the benefit of semantic operations over repeated framework boilerplate.

## Dependencies

M8 and M6; NIL-082, NIL-062.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-090 | Choose and freeze one domain task, likely a local HTTP route returning a typed response; define core/plugin/framework vocabulary tracks. | NIL-082, NIL-062 |
| NIL-091 | Implement the minimal domain plugin and higher-level semantic operation with explicit runtime/effect contracts. | NIL-090 |
| NIL-092 | Build equivalent conventional-framework baseline and integration oracle; publish token/context/repair comparison. | NIL-091 |

## Tests

Local integration tests for successful and invalid requests, status/body semantics, lifecycle cleanup, declared effects and runtime failures.

## Benchmark requirements

Core-only, low-level plugin and semantic framework results separately; include runtime/library/schema costs and baseline facilities.

## Deliverables

One functioning extension, reproducible integration fixtures and paired benchmark report.

## Acceptance criteria

The same domain behavior passes all baseline and NIL tests; semantic compression is measured without attributing library abstraction to syntax.

## Explicitly excluded work

Complete web stack, authentication system, database framework, production deployment and multiple domains.

## Risks and open questions

Domain choice and runtime dependencies need explicit scope; network capabilities must be constrained in generated-code tests.

## 2026-09-30 audit

Retain this as a semantic-compression experiment, not a prerequisite for the core
language. HTTP is a candidate; select one domain with a baseline and oracle after
M8. No web/database framework or runtime exists. See the [audit](../MILESTONE_AUDIT.md).
