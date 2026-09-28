# Milestone 0 — Research consolidation

Status: **Complete for setup scope**.

## Objective

Turn the research into a scoped, reviewable engineering contract.

## Motivation

Avoid treating speculative examples and conflicting reports as a finalized specification.

## Dependencies

None.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-001 | Inventory all repository research and preserve originals; record conflicts and source provenance in PROJECT_STATE.md. | none |
| NIL-002 | Write architecture, proposed core, ADRs and benchmark methodology; distinguish accepted constraints from provisional policies. | NIL-001 |
| NIL-003 | Create workspace/CLI scaffold, README and CI; verify formatting, lint and tests; record actual delivery status. | NIL-002 |

## Tests

Review every research conflict against PROJECT_STATE.md; ensure all commands advertised as available actually work.

## Benchmark requirements

Freeze TCR/TTCP definitions, failure handling and measurement provenance; no token-saving claims.

## Deliverables

PROJECT_STATE.md, ROADMAP.md, architecture overview, ADRs, milestone plans, benchmark protocol, buildable scaffold.

## Acceptance criteria

All known research is inventoried; scope and uncertainty are explicit; scaffold checks pass.

## Explicitly excluded work

Compiler semantics implementation, tokenizer tools and model calls.

## Risks and open questions

External citation handles cannot substantiate reported results; no existing commit conventions. Latest user instruction limits delivery to setup/planning.

## Delivery record

Completed setup and planning on 2026-09-28. Read both research reports completely;
preserved them and AGENTS.md. Created 13 milestone plans, 10 ADRs, architecture/core
proposals, benchmark methodology and one dependency-free CLI crate. No compiler
implementation tasks were started after the user narrowed scope.

Validation: `cargo build --workspace --locked --offline`,
`cargo test --workspace --locked --offline` (3 integration tests),
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`,
and `cargo fmt --all -- --check` passed. Help/version commands work. All generated
local Markdown links and required milestone sections were checked. Local toolchain:
Rust 1.95.0-nightly (2026-02-10); no nightly language features used. Stable and 1.85
checks are configured in CI but were not run locally; hosted CI has not run yet.

Next action: NIL-010 in M1 when implementation is requested. No model calls, paid
experiments, publishing or later milestone work occurred.
