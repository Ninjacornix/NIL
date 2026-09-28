# NIL engineering roadmap

## Scope and status

This roadmap replaces speculative calendar dates with dependency-driven acceptance
criteria. The latest instruction requests setup and concrete plans only. M0 is
complete for that scope; M1–M12 are unimplemented. The CLI currently provides help
and version information, not parsing or execution. Do not mark a future milestone
complete because its directory or specification exists.

Each linked plan contains objective, motivation, dependencies, task IDs, tests,
benchmark requirements, deliverables, acceptance criteria, exclusions and risks.

| Milestone | Detailed plan | Status |
|---|---|---|
| 0 | [Research consolidation](milestones/00-research.md) | Complete for setup scope |
| 1 | [Minimal executable NIL](milestones/01-minimal-executable.md) | Planned; not implemented |
| 2 | [Control flow](milestones/02-control-flow.md) | Planned; not implemented |
| 3 | [Evidence-driven types](milestones/03-types.md) | Planned; not implemented |
| 4 | [Canonical semantic IR](milestones/04-canonical-ir.md) | Planned; not implemented |
| 5 | [Token benchmark infrastructure](milestones/05-tokenbench.md) | Planned; not implemented |
| 6 | [LLM generation benchmark](milestones/06-generation-benchmark.md) | Planned; not implemented |
| 7 | [Syntax experiments](milestones/07-syntax-experiments.md) | Planned; not implemented |
| 8 | [Minimal plugin ABI](milestones/08-plugin-abi.md) | Planned; not implemented |
| 9 | [First semantic framework](milestones/09-semantic-framework.md) | Planned; not implemented |
| 10 | [Native execution and optimization](milestones/10-native-backend.md) | Planned; not implemented |
| 11 | [Model-specific representation](milestones/11-model-representation.md) | Planned; not implemented |
| 12 | [Self-hosting investigation](milestones/12-self-hosting.md) | Planned; not implemented |

## Execution order

Default sequence: M0 → M1 → M2 → M3 → M4 → M5 → M6 → M7 → M8 → M9 → M10 → M11 → M12.
Dependencies permit source measurement (NIL-050) after M1, native investigation after
M4, and plugin work after the type/IR contract. These are options, not authorization
to start later work. M11 and M12 remain investigations with go/no-go gates.

Start implementation with NIL-010. The immediate executable gate is arithmetic
across a call returning 42, with lexer/parser, malformed input, type checking, HIR
validation/lowering, diagnostics and execution tests. See the [proposed core](language/V0_1.md).

## Completion discipline

Record task status, checks performed, actual commands, changed assumptions and new
regression cases in the milestone file at delivery. Every fixed compiler bug gets a
regression test. Use Rust unit/integration tests and small checked-in golden fixtures;
add fuzz/property infrastructure after the parser and validators stabilize, before
accepting hostile generated workloads at scale. No arbitrary coverage percentage
replaces tests of semantic invariants and failure paths.

## Discoveries from this pass

- Research exists only in two documents; there is no pre-existing compiler or test suite.
- Prefix trees versus implicit-result lines is unresolved; the initial profile is provisional.
- Production backend choice is open; interpreter-first is supported by the report.
- M2 necessarily introduces bool for comparisons ahead of the broader M3 type work.
- Original research claims and embedded citation handles need source recovery before reuse.
- Setup has no tokenizer/model dependency and makes no token-efficiency claim.
