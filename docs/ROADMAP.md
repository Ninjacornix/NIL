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

## Application-language workstream

User-selected target: runtime-sized data, files and text. The first executable
scope is implemented in expr-v5: dynamic i64 buffers, bytes/UTF-8 literals,
sequence operations, decimal conversion and explicit file/stdout effects.
The default and earlier profiles remain compatible. See [the concrete development
plan](GENERAL_PURPOSE.md) for remaining ownership/views, records/types, error,
capability and integration decisions. C/driver parity is not claimed.
Acceptance programs include runtime buffers beyond 256 elements, text processing,
binary file copy, dynamic calls/loops and matching reference/native failure cases.
No benchmark syntax winner or production memory ABI is selected by this work.

## Expr-v5 reclaimable storage — completed workstream

Reclaim dead native arena entries, drop dead reference values, and generalize
replacement-chain proofs to Buffer/Bytes with last-use and runtime alias checks.
The quota changed to live/transient storage; its 64 MiB value is unchanged.
Alias-hostile differential cases, sanitizer coverage, IR selection tests and a
full 1 MiB file transform validate this work. The measured file-loop benchmark
shows NIL remains slower than C++; profiling/root-call/I/O optimization remains
open. Views, ownership syntax, new types and plugin ABI work are excluded.
See [actual command evidence](validation/DYNAMIC_STORAGE.md).

## Expr-v5 checked runtime speed — completed workstream

Bulk regular/streamed file reads, whole-program inlining of checked C accessors,
loop-invariant length hoisting and root-slot retention remove the attributed
per-byte I/O and shadow-stack costs. Cached live bytes/root counts retain quota
and alias checks without rescanning the arena. New read-boundary/pipe/root tests,
three seeded differential campaigns and sanitizers validate the same semantics.
See [command evidence](validation/APPLICATION_RUNTIME.md), [ADR 018](adr/018.md)
and [real before/after stage measurements](../benchmarks/reports/2026-10-02/APPLICATION_RUNTIME.md).
Further performance work should use specific measured kernels; new types, views,
unsafe profiles, plugin ABI and changes to the quota/default are excluded here.

## Expr-v5 incremental builders — completed workstream

Separate logical length from reserved capacity and extend proven dead concat
operands in place. Geometric growth is clamped to the steady-state transient
budget; capacity-aware quota accounting intentionally shifts E013 boundaries.
Copy when aliases, future reads or self-concat prevent uniqueness. Preserve ordered
checks/effects and immutable values. Extend the independent O0/O2 differential
oracle with append-hostile families, verify realloc/root forwarding under sanitizers,
and assert both emitted IR paths. Measure append scaling, text-report formatting,
file newline counting and integer reduction against C++/Python, then remeasure the
file transform. See [validation](validation/APPLICATION_BUILDERS.md) and
[all measured outcomes](../benchmarks/reports/2026-10-02/APPLICATION_BUILDERS.md).
Further optimization must target measured losses; slices/views, ownership syntax,
new types, plugin ABI and raising the quota remain excluded from this workstream.

## Expr-v5 scalar lazy-region roots — completed workstream

Prove recursively nonallocating scalar arms, preserve capture-union liveness and
borrow region inputs without duplicate roots. Accept these arms in loop root
retention; preserve lazy CFG, traps, side effects and instruction budgets. Validate
O0/O2 unselected traps/effects, effect order and allocations after last-use borrows,
then ASan/UBSan and three seeded 64-family differential campaigns. Remeasure every
builder/transform workload plus byte-sum and scalar-callee scan controls. See
[command evidence](validation/LAZY_REGIONS.md), [actual IR](architecture/SCALAR_LAZY_IR.md)
and [measurements](../benchmarks/reports/2026-10-03/LAZY_REGIONS.md).
Interprocedural/intrinsic summaries, sequence-returning arms and nested-loop proofs
remain separate work; conservative handling stays where this proof cannot apply.


## Expr-v5 interprocedural borrowing — completed workstream

Compute validated HIR summaries before LLVM lowering; admit scalar nonallocating,
non-host-effect callees and recursive groups. Extend borrowing to known checked
reads/parse and nested identity-state loops. Keep sequence-producing or allocating
calls conservative. Cover self/mutual/depth-limit recursion, called lazy traps and
effects, recursive sequence liveness and emitted call/root assertions at O0/O2.
Extend differential generation to 72 families, validate three seeds and sanitizers,
and measure leaf, forced-noinline, recursive and nested controls plus the entire
append/transform workload set. See [validation](validation/CALL_ROOTS.md),
[IR](architecture/CALL_ROOTS_IR.md) and
[all measurements](../benchmarks/reports/2026-10-03/CALL_ROOTS.md).
No syntax, ownership, profile, plugin or quota expansion accompanies this proof.
