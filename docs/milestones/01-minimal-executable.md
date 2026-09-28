# Milestone 1 — Minimal executable NIL

Status: **Planned; not implemented**.

## Objective

Execute arithmetic across typed function calls, including add(20, 22) → 42.

## Motivation

Establish the smallest independently testable semantic pipeline before broad language design.

## Dependencies

M0; NIL-003.

## Implementation tasks

Tasks are ordered by explicit dependencies. Each task should be delivered as a small,
reviewable change with its tests and specification updates. Do not start a dependent
task until its contract is available.

| ID | Work | Depends on |
|---|---|---|
| NIL-010 | Create nil-hir with Type::I64, typed value/function IDs, signatures, semantic instruction enums and structured diagnostics. Define validated-program ownership. | NIL-003 |
| NIL-011 | Create nil-compiler syntax module and byte-spanned line lexer/parser for the proposed lines-v0 grammar. Reject unknown/trailing tokens, bad integers, missing terminators and extra returns. | NIL-010 |
| NIL-012 | Resolve all signatures before bodies; lower AST labels to HIR IDs; check duplicate labels, operands, call arity and return types. Add independent HIR validation for alternate frontends. | NIL-011 |
| NIL-013 | Implement checked i64 evaluator with explicit call frames, instruction fuel and depth limits; separate runtime traps from static diagnostics. | NIL-012 |
| NIL-014 | Add nil check/run/hir commands, examples/add.nil and integration tests; add frontend/runtime timing bench and document exact commands. | NIL-013 |

## Tests

Unit-test token spans, CRLF/EOF, integer bounds, malformed directives and unsupported types. Test unknown/duplicate functions, forward references, bad operand order/arity/return, invalid hand-built HIR, deterministic lowering, nested calls, all four arithmetic operations, signed division, overflow, zero division and budget exhaustion. Golden-test one diagnostic and HIR fixture. CLI tests cover exit codes, I/O errors and result 42. Test debug and release.

## Benchmark requirements

Record source bytes/characters, frontend latency and interpreter runtime separately; tokenizer/native metrics null. No speed threshold before stable baseline hardware.

## Deliverables

Two real library crates, working CLI pipeline, fixtures, test suite, language spec reconciled with code, timing harness.

## Acceptance criteria

Fresh checkout builds offline with no external Rust dependencies; source → AST → checking → validated HIR → evaluation produces 42 across a function call. Malformed input produces structured errors without panic; formatting/lint/tests pass.

## Explicitly excluded work

Branches, booleans, loops, memory, plugins, MIR implementation, optimizers, native/WASM codegen, canonical serializer and syntax comparisons.

## Risks and open questions

Line syntax and trapping i64 policy are provisional. Recursion is structurally valid but bounded; do not confuse execution limits with language semantics.
