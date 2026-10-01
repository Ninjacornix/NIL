# NIL engineering roadmap

## Scope and status

The [2026-09-30 audit](MILESTONE_AUDIT.md) reconciles the original plans with
`master` at `c023b4e`. Milestone numbers identify workstreams; they are not a mandatory
linear sequence. Existing features should not be reimplemented to match a proposed
folder layout. The research objective remains failure-inclusive tokens to a correct
program, separate from raw source tokens and native runtime.

| Milestone | Detailed plan | Current status |
|---|---|---|
| 0 | [Research consolidation](milestones/00-research.md) | Complete for initial scope |
| 1 | [Minimal executable NIL](milestones/01-minimal-executable.md) | Complete |
| 2 | [Control flow](milestones/02-control-flow.md) | Complete |
| 3 | [Evidence-driven types](milestones/03-types.md) | Complete for selected expr-v4 bool/array scope; proven sparse updates optimized |
| 4 | [Canonical semantic IR](milestones/04-canonical-ir.md) | Partial: validated HIR and LLVM SSA; canonical serialization absent; portable MIR conditional |
| 5 | [Token benchmark infrastructure](milestones/05-tokenbench.md) | Core source measurement delivered; full schema/corpus plan partial |
| 6 | [LLM generation benchmark](milestones/06-generation-benchmark.md) | Local runner implemented; pilot stopped, full study pending |
| 7 | [Syntax experiments](milestones/07-syntax-experiments.md) | Profiles, equivalence tests and token studies delivered; model/repair study pending |
| 8 | [Minimal plugin ABI](milestones/08-plugin-abi.md) | Unimplemented; design seam exists |
| 9 | [First semantic framework](milestones/09-semantic-framework.md) | Deferred until plugin and measurement gates |
| 10 | [Native execution and optimization](milestones/10-native-backend.md) | Delivered for current core; production hardening and extra backends separate |
| 11 | [Model-specific representation](milestones/11-model-representation.md) | Deferred, evidence/budget gated |
| 12 | [Self-hosting investigation](milestones/12-self-hosting.md) | Optional late investigation |

## Execution priorities

1. Extend existing paired measurement infrastructure with held-out task manifests
   and versioned attempt/usage records; do not recreate the current tokenizers/runners.
2. Maintain the optional M6 runner and offline mocks. The local model pilot was
   stopped; defer further inference until a suitable training/adaptation protocol
   is chosen. Canonical serialization and extra types do not block the tooling.
3. Review the selected M3 expr-v4 array/bool experiment and its paired benchmarks.
   Preserve profile compatibility; require motivating programs and contracts before
   adding widths, floats or references. The broader memory contract remains open.
4. Complete M4 canonical serialization if required by corpus interchange/tooling.
   Introduce a separate MIR only with an independent consumer or concrete compiler need.
5. Investigate M8/M9 after defining typed operation/effect contracts and a measurable
   semantic-compression task. Keep M11/M12 conditional on evidence and value.

These priorities recommend scope; they do not authorize model spending, framework
implementation or later language features. Detailed task IDs remain in the linked
plans, with implemented portions and conditional dependencies recorded explicitly.

## Delivered compiler and experiments

The pipeline is source profile → AST → type checking → validated HIR → LLVM SSA
lowering → host native executable, with a bounded reference evaluator as oracle.
Profiles are lines-v0 and expr-v0/v1/v2/v3/v4; expr-v0 remains default. V3 changes
arithmetic/accounting policy, not v2's grammar. V4 extends v3 with typed function
signatures and immutable fixed-size integer arrays. Source-token, frontend/backend,
runtime and binary-size measurements exist; LLM correctness/repair/TCR measurements do not.

[Fuzz testing](FUZZING.md) provides source/HIR mutation, independently interpreted
valid programs, O0/O2 differential execution, deadlines and reproducers. CI runs
smoke properties and nightly expands the campaign. Coverage-guided fuzzing and
shrinking remain improvements rather than missing initial compiler functionality.

## Completion discipline

Retain original reports and dated delivery records. Mark partial acceptance criteria
explicitly; documented designs and debug dumps are not stable serialized APIs.
Every fixed compiler bug receives a regression test. Review golden diagnostics/IR,
run relevant checks, and record actual commands and provenance. Test counts and
fuzz campaigns do not establish compiler completeness, universal speed or LLM efficiency.

See [project state](PROJECT_STATE.md), the [audit evidence map](MILESTONE_AUDIT.md#evidence-map),
and the [failure-inclusive benchmark protocol](../benchmarks/README.md).

## M3 delivery and native storage follow-up

Typed bool functions and immutable fixed arrays are implemented. The historical
[expr-v4 report](../benchmarks/reports/2026-09-30/EXPR_V4.md) identified
quadratic loop copying; [ADR 015](adr/015.md) records the implemented def-use proof
and private storage lowering. Conditional replacement chains preserve lazy checks,
aliases, old reads and simultaneous state updates. Unsupported chains retain value
lowering. Typed internal functions permit aggregate bridge optimization.

The final performance gate is per-kernel NIL/C++ ≤1.25 against fresh-local C++,
with identical drivers, independently checked fixtures and repeated interleaved
measurements. The storage report records completion evidence. Source token counts
remain separate from model-generation success and repair/TCR, which are unmeasured.
No reference type, second backend or surface grammar expansion was required.

Final evidence: [storage benchmark](../benchmarks/reports/2026-09-30/EXPR_V4_STORAGE.md) — all 34 kernels pass the 1.25× C++ gate;
combined source uses 1115/1194 NIL versus 1425/1425 Python tokens. Generation/TCR
remains unmeasured.

## Local model trajectory pilot

[The generation runner](../benchmarks/generation/README.md) now compares existing
checked-semantic profiles with frozen local Gemma/Qwen model access, repair budgets
and reference/native oracles. Raw output/input usage and failure-inclusive TCR
replace guesses about model efficiency. Finish and audit all paired cells before
selecting a representation; retain the default when the gates are inconclusive.
