# NIL project state

## Sources and scope

Read in full: [design.md](about/misc/design.md) (1,456 lines) and
[report.md](about/misc/report.md) (2,151 lines). These are research proposals,
not an approved language specification. At inspection the repository contained
only those documents, AGENTS.md, and an initialized Git repository with no commits.
The original documents and AGENTS.md are preserved. The current user brief controls
scope where it differs from the research.

## Confirmed direction

- Optimize total model work until semantic correctness, not characters or only
  final-source tokens. Preserve raw tokenizer counts as a separate measurement.
- Rust compiler; deterministic parsing and strong static checking; a small core.
- Separate experimental source profiles from a typed semantic HIR. Let the compiler
  generate value IDs and, eventually, control-flow bookkeeping.
- Typed plugin operations may extend semantics, never introduce arbitrary grammar.
- Keep a reference interpreter for semantic tests; postpone production codegen.
- Evaluate representations using paired tasks, exact tokenizers, hidden tests,
  repair trajectories, and failure-inclusive budgets.

## Conflicts and disposition

| Question | Evidence / conflict | Engineering disposition |
|---|---|---|
| Generated form | design favors nested prefix trees; report favors implicit-result instruction lines | Use report-style lines for the first executable experiment; neither is selected as final. Compare in M7. |
| SSA | Both reject requiring model-written SSA, but report's straight-line implicit IDs are SSA-like | Immutable HIR values in M1; no requirement for surface SSA or future HIR CFGs. |
| External evidence | design cannot verify Lingo/toke; report quotes concrete results for them; kernl counts receive different qualifications | Treat all these figures as unverified here. Recover primary sources before baseline inclusion. Embedded citation handles are not usable bibliography links. |
| Backend | design permits early direct LLVM; report explicitly prioritizes interpreter, later MLIR/LLVM | Interpreter now; native choice open (ADR-002). |
| Types and vocabulary | Research proposes much wider MVPs and sometimes i32, sometimes i64 | User limits M1; choose only i64 provisionally. No general inference. |
| Schedule | Research proposes calendar dates, 500-test gates, early measurement tools and plugins | Use acceptance-driven M0–M12; freeze methodology now, implement full tokenbench at M5. No unsupported calendar commitment or test-count proxy. |
| Canonical syntax | design says one syntax; report allows model profiles | One spelling per operation within each versioned profile; separate experimental frontends later. |

## Current delivery status

M0 and M1 are complete. The user authorized implementation after the setup pass.
The three-crate workspace now parses lines-v0, resolves/type-checks all functions,
validates syntax-independent HIR and executes it with a bounded reference evaluator.
The CLI supports check/run/hir. M2–M12 remain planned as full milestones.
An early `expr-v0` source profile prototype now lowers to the same HIR, with
paired source-token and interpreter timing fixtures. This is partial M7 work;
model-generation trajectories and syntax selection are still open.

## Working assumptions and architecture

M1: text → syntax AST → signature resolution/type checking/lowering → validated
HIR → evaluator. Three crates separate semantic IR, compiler/frontend/execution,
and CLI. HIR has typed immutable values, pure calls, and one return per function.
MIR/SSA CFG lowering is a later boundary, not an empty crate today.

The M1 `i64` arithmetic traps on overflow and invalid division. Evaluation has explicit
instruction and call-depth budgets. These are documented prototype policies,
not a stable language ABI. Syntax is `lines-v0`, not a token-efficiency claim.

## Terminology

**Profile:** replaceable source encoding. **AST:** syntax with source byte spans.
**HIR:** syntax-independent typed semantic operations. **MIR:** future lowered
control-flow representation. **SSA:** each value defined once. **Semantic plugin:**
a typed instruction set. **TCR:** failure-inclusive aggregate output cost per solved
trial. **TTCP/TTC:** per-trial tokens through first correct candidate.

## Open questions and postponed work

Target models/tokenizers, syntax winner, final widths and overflow policy, memory
safety/ownership, structured control flow, effect model, plugin version/ABI rules,
canonical serialization, native backend, and success thresholds remain open.
No macros, standard library, package manager, IDE/LSP, framework, custom tokenizer,
training, native optimization, or self-hosting in this pass. Research percentages
and suggested 30% savings are hypotheses, not NIL results.
