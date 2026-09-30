# NIL engineering roadmap

## Scope and status

This roadmap replaces speculative calendar dates with dependency-driven acceptance
criteria. M0 setup, M1 and M2 implementation are complete. M3–M6, M8–M9 and M11–M12 remain unimplemented. M10 has a user-authorized LLVM-native default path; broader backend evaluation remains outstanding. M7 has an expression profile and small paired source-token study; model generation, repair trajectories and broader syntax comparisons remain outstanding.
M1 and M2 are complete and M7 has a default expression profile plus a small source-token
study. Model-generation, repair-trajectory and expanded syntax comparisons remain open.

Each linked plan contains objective, motivation, dependencies, task IDs, tests,
benchmark requirements, deliverables, acceptance criteria, exclusions and risks.

| Milestone | Detailed plan | Status |
|---|---|---|
| 0 | [Research consolidation](milestones/00-research.md) | Complete for setup scope |
| 1 | [Minimal executable NIL](milestones/01-minimal-executable.md) | Complete |
| 2 | [Control flow](milestones/02-control-flow.md) | Complete |
| 3 | [Evidence-driven types](milestones/03-types.md) | Planned; not implemented |
| 4 | [Canonical semantic IR](milestones/04-canonical-ir.md) | Planned; not implemented |
| 5 | [Token benchmark infrastructure](milestones/05-tokenbench.md) | Planned; not implemented |
| 6 | [LLM generation benchmark](milestones/06-generation-benchmark.md) | Planned; not implemented |
| 7 | [Syntax experiments](milestones/07-syntax-experiments.md) | Partial: expression profile is default; generation study pending |
| 8 | [Minimal plugin ABI](milestones/08-plugin-abi.md) | Planned; not implemented |
| 9 | [First semantic framework](milestones/09-semantic-framework.md) | Planned; not implemented |
| 10 | [Native execution and optimization](milestones/10-native-backend.md) | Partial: LLVM native default, current-core differential tests and measurements |
| 11 | [Model-specific representation](milestones/11-model-representation.md) | Planned; not implemented |
| 12 | [Self-hosting investigation](milestones/12-self-hosting.md) | Planned; not implemented |

## Execution order

Default sequence: M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 → M11 → M12.
Dependencies permit source measurement (NIL-050) after M1, native investigation after
M4, and plugin work after the type/IR contract. These are options, not authorization
to start later work. M11 and M12 remain investigations with go/no-go gates.

NIL-010 through NIL-014 are complete. The verified executable gate is arithmetic
across a call returning 42, with lexer/parser, malformed input, type checking, HIR
validation/lowering, diagnostics and execution tests. See the [M1 core](language/V0_1.md).

## Completion discipline

Record task status, checks performed, actual commands, changed assumptions and new
regression cases in the milestone file at delivery. Every fixed compiler bug gets a
regression test. Use Rust unit/integration tests and small checked-in golden fixtures;
add fuzz/property infrastructure after the parser and validators stabilize, before
accepting hostile generated workloads at scale. No arbitrary coverage percentage
replaces tests of semantic invariants and failure paths.

## Discoveries from this pass

- Research exists only in two documents; there was no pre-existing compiler or test suite at initial inspection.
- Prefix trees versus implicit-result lines remains unresolved; expr-v0 is a reversible default backed by a small source-token comparison.
- Production backend choice is open; interpreter-first is supported by the report.
- M2 necessarily introduces bool for comparisons ahead of the broader M3 type work.
- Original research claims and embedded citation handles need source recovery before reuse.
- The local paired token study covers three arithmetic examples and does not measure model generation or TCR.

NIL-020 through NIL-022 are complete: typed bool comparisons, lazy structured
branches, one state-tuple loop, bounded execution and four acceptance programs.
See [M2 semantics](language/CONTROL_FLOW.md) and the linked milestone evidence.
