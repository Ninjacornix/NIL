<!-- Title: feat(syntax): make expression syntax the default
Head: feat/expr-profile-default
Base: master
Merge method: Squash and merge
-->

## Purpose

The line-oriented prototype requires separate instructions for simple arithmetic. Make `expr-v0` the default CLI and library profile so programs can express arithmetic and function calls directly. Preserve `lines-v0` as an explicit compatibility option.

## Changes

- Route public parsing and compilation through explicit source profiles.
- Update the CLI default and convert the addition example to expression syntax.
- Add arithmetic examples and solutions for Count Odd Numbers, Smallest Even Multiple, and Calculate Money in Leetcode Bank.
- Test lexer byte spans, default-profile selection, malformed input, equivalent HIR, and example results across problem domains.

## Validation

```text
Complete-stack checks at ab37194 (not separate reruns of this branch head):
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.

Relevant coverage includes CLI integration, expression/legacy parser equivalence,
compile-fail diagnostics, type checking, and independent HIR validation.
The expression and legacy fixtures retain equivalent HIR; no HIR golden file
changes in this PR. MIR/native execution is not introduced by this PR.
```

## Issues and compatibility

Implicit parsing changes from `lines-v0` to `expr-v0`. Existing line programs must select `--profile lines-v0` or `SourceProfile::LinesV0`; arithmetic semantics remain checked. This changes the prototype default without settling final syntax or demonstrating model-generation efficiency.

See [expression specification](https://github.com/Ninjacornix/NIL/blob/feat/expr-profile-default/docs/language/EXPR_V0.md) and [ADR 006](https://github.com/Ninjacornix/NIL/blob/feat/expr-profile-default/docs/adr/006.md). No linked issue.
