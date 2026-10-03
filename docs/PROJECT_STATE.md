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
- Keep a reference interpreter for semantic tests; LLVM native execution is now the user-selected default.
- Evaluate representations using paired tasks, exact tokenizers, hidden tests,
  repair trajectories, and failure-inclusive budgets.

## Conflicts and disposition

| Question | Evidence / conflict | Engineering disposition |
|---|---|---|
| Generated form | design favors nested prefix trees; report favors implicit-result instruction lines | Started with report-style lines, then made expr-v0 the default after paired source-token measurements favored it on three arithmetic cases. Keep both profiles; broader generation/TCR evidence is still required in M7. |
| SSA | Both reject requiring model-written SSA, but report's straight-line implicit IDs are SSA-like | Immutable HIR values in M1; no requirement for surface SSA or future HIR CFGs. |
| External evidence | design cannot verify Lingo/toke; report quotes concrete results for them; kernl counts receive different qualifications | Treat all these figures as unverified here. Recover primary sources before baseline inclusion. Embedded citation handles are not usable bibliography links. |
| Backend | design permits early direct LLVM; report explicitly prioritizes interpreter, later MLIR/LLVM | LLVM native default after user authorization (ADR-012); interpreter retained as oracle. |
| Types and vocabulary | Research proposes much wider MVPs and sometimes i32, sometimes i64 | User limits M1; choose only i64 provisionally. No general inference. |
| Schedule | Research proposes calendar dates, 500-test gates, early measurement tools and plugins | Use acceptance-driven workstreams; source measurement is delivered in benchmarks/paired. Extend attempt accounting for M6. No unsupported calendar commitment or test-count proxy. |
| Canonical syntax | design says one syntax; report allows model profiles | One spelling per operation within each versioned profile; separate experimental frontends later. |

## Current delivery status

M0, M1 and M2 are complete. The workspace defaults to expr-v0 and also
supports lines-v0, expr-v1, expr-v2 and opt-in expr-v3/v4. Frontends resolve/type-check all functions
and validate syntax-independent HIR. The CLI supports check/hir/llvm/build/run;
run/build use LLVM natively, while the library retains a bounded reference evaluator.
M3 adds explicitly typed bool/array functions and immutable fixed-size i64 arrays
in expr-v4; floats, other widths and explicit memory remain deferred. M4 canonical
serialization is absent; a local M6 generation/repair runner exists; its pilot was interrupted.
M5 source measurement and M7 profile experiments are implemented to useful extents;
M10 native execution is delivered for the current core. Plugins remain unimplemented;
model adaptation and self-hosting remain conditional investigations. The
[milestone audit](MILESTONE_AUDIT.md) distinguishes delivered capabilities from
remaining original-plan requirements.
The expr-v0 default was chosen after the local paired benchmark showed lower source-token counts for three arithmetic programs on two pinned tokenizers, while execution lowered to identical HIR. This is partial M7 work; model-generation trajectories and broader syntax selection are still open.

## Working assumptions and architecture

Current pipeline: text → syntax AST → signature resolution/type checking/lowering →
validated HIR → backend SSA CFG → LLVM → native executable. Four compiler crates separate
semantic IR, compiler/frontend/reference execution, native backend and CLI; a fifth
workspace package provides seeded fuzz testing. HIR has
typed immutable values, pure calls, structured regions and one return per function.
The backend block builder is internal; canonical serialization remains absent.
A separate portable MIR is conditional on a concrete consumer/compiler need.

Earlier profiles use checked `i64` arithmetic with overflow/division traps. Expr-v3
uses wrapping arithmetic, defined MIN/-1 division and no default native accounting;
division by zero still traps. Reference evaluation has explicit instruction and
call-depth budgets. These are documented prototype policies,
not a stable language ABI. `expr-v0` is the default surface profile; source syntax remains experimental and no token-efficiency claim generalizes beyond the recorded paired corpus.

## Terminology

**Profile:** replaceable source encoding. **AST:** syntax with source byte spans.
**HIR:** syntax-independent typed semantic operations. **MIR:** future lowered
control-flow representation. **SSA:** each value defined once. **Semantic plugin:**
a typed instruction set. **TCR:** failure-inclusive aggregate output cost per solved
trial. **TTCP/TTC:** per-trial tokens through first correct candidate.

## Open questions and postponed work

Target models/tokenizers, syntax winner, final widths and overflow policy, memory
safety/ownership, effect model, plugin version/ABI rules, canonical serialization,
production hardening/portability needs, and success thresholds remain open. LLVM is
the selected current backend; building additional backends is not an immediate gate.
No macros, standard library, package manager, IDE/LSP, framework, custom tokenizer,
training, custom native optimization passes, or self-hosting in this pass. Research percentages
and suggested 30% savings are hypotheses, not NIL results.

## M2 update

ADR 011 implements structured branches and state-tuple loops with immutable typed
region values, bool conditions, parallel loop updates, scope validation and an
explicit evaluator stack. Expression profiles now have control flow; lines-v0 is
unchanged compatibility syntax. Function source signatures remain i64; bool is
available inside expressions and loop state. General types and portable MIR remain postponed; the later LLVM update below
adds native codegen and standard LLVM optimization. Expr-v2 is opt-in; eight control-flow samples
use fewer raw tokens than paired Python under cl100k_base and pinned Qwen, without
establishing model-generation/TCR or runtime superiority.

## LLVM native default

The user explicitly requested switching from interpretation to LLVM compilation.
`nil run` compiles and executes native code; `nil build` creates a reusable executable;
`nil llvm` exposes the generated function module. nil-llvm consumes only validated
HIR and preserves checked arithmetic, lazy regions, parallel loop updates and exact
fuel/depth semantics. Clang is an explicit dependency with no interpreter fallback.
Hosts: 64-bit macOS/Linux. Typed LLVM SSA/phi lowering is implemented; portable MIR
canonicalization and complete M4/M10 backend evaluation remain future work.

## expr-v3 experiment

The user requested C++-class native speed with compact source. ADR 013 adds opt-in
expr-v3: v2's exact grammar, explicit HIR wrapping arithmetic, defined MIN/-1 division
and no default native resource accounting. Earlier semantics are preserved. Bounded
instrumentation remains available; reference execution is bounded. This is partial
M7/M10 work, not completion of the broader type/IR milestones. Overflow-mistake
detection and generation/repair TCR remain unresolved tradeoffs, so v3 is not silently
made the default. Benchmarks record both ordinary and checked C++ baselines.

## expr-v3 resilience testing

A dependency-free nil-fuzz workspace tool now mutates source and HIR, generates
well-typed terminating programs and compares a separate tree oracle with reference
and LLVM execution. Native O0/O2, optional budgets, recursion/helpers and nested bool
loop state are exercised. CI runs seeded smoke properties; nightly expands the campaign
and archives reproducers. This is mutation/property fuzzing, not coverage-guided or
exhaustive verification. See [FUZZING.md](FUZZING.md).

## M3 expr-v4 update

[ADR 014](adr/014.md) selects bool function boundaries and immutable fixed-length
i64 arrays for sum/dot/max/search/transform programs. Lengths 0..256 are types;
indexing/replacement trap with E012. Array values can cross calls, branches and
loop state. No implicit conversions, exposed pointers, allocation or effects.
V4 retains v3 wrapping arithmetic and optional native budgets; earlier profiles
and the expr-v0 default are unchanged. Typed native entries flatten arguments
through an LLVM bridge without depending on C aggregate ABI equivalence.

The reference evaluator, independent HIR validator and LLVM lowering implement
the same operations. Seeded array/oracle checks, source/HIR mutations and native
O0/O2 comparisons accompany the experiment. [Full-program Python/C++ results](../benchmarks/reports/2026-09-30/EXPR_V4.md)
record source tokens, checker/payload cost, compilation, size and runtime. Read-only
parameter snapshots avoid repeated copying. [ADR 015](adr/015.md) adds private
loop storage for proven single-use replacement chains, including conditional
updates, and internal typed-function inlining. Writes commit after body evaluation
while preserving aliases, lazy bounds checks and simultaneous updates. Unproven
chains retain value copying. Final storage measurements compare fresh-local
C++/Python rather than relying on explicit-copy baselines. Choosing the best
representation for model generation still requires correctness/repair/TCR data.

## Local Ollama pilot

The [trajectory runner](../benchmarks/generation/README.md) compares v0/v1/v2 under
matched semantics using installed Gemma 3 4B and Qwen 2.5 7B Instruct. It retains
raw attempts, real provider usage, failed trials and native/reference validation.
The pilot was stopped at the user’s request. Full paired results and declared confidence gates must be audited before choosing
a source representation. This numerical-core pilot does not complete the broader
model/baseline study.

## Application core update

The maintainer selected application programs (runtime data, files/text) rather than
raw systems access. Opt-in expr-v5 adds immutable dynamic i64 buffers, byte/text
values, typed builtins and explicit host effects. See [v5](language/EXPR_V5.md),
[ADR 017](adr/017.md) and [the application plan](GENERAL_PURPOSE.md).
Native LLVM and the reference path implement the same sequence semantics and
live allocation charges with transient result reservations. Reference host I/O requires explicit opt-in;
application executables use ordinary OS permissions. Existing profiles/default
remain unchanged. These builtins are not a full plugin registry. No new token,
TCR or runtime-speed conclusion follows from this implementation.
Records, widths/floats, recoverable errors, ownership/views and external ABI remain
open; older descriptions of pure functions apply only to the earlier core.

V5 storage now reclaims dead SSA allocations and reuses proved single-use dynamic
replacement chains with runtime alias protection. The 64 MiB quota meaning changed
from cumulative to live storage. File-transform measurements show feasibility,
not C++ runtime parity; see [storage evidence](validation/DYNAMIC_STORAGE.md).

V5 runtime optimization adds bulk `fread` (regular-size hints, streaming fallback),
O2 LTO checked C accessors, preserved-length hoisting and conservative root-slot
retention. Root-count/live-byte caches avoid whole-root scans at each replacement.
[ADR 018](adr/018.md) records the decision; [runtime evidence](validation/APPLICATION_RUNTIME.md)
and [before/after measurements](../benchmarks/reports/2026-10-02/APPLICATION_RUNTIME.md)
separate measured file-loop speed from unproven general C++ parity. Quota/trap/alias
semantics and earlier profiles remain unchanged.

Single-use concat now extends bytes/buffers with geometric spare capacity. The
last-use check plus runtime uniqueness prevents mutations through retained aliases;
self-concat copies. Capacity is distinct from length and is charged at width ×
capacity + 40 bytes per allocation, deliberately changing E013 boundaries while
retaining the 64 MiB limit. Copy and reuse reserve identical transient capacities.
See [updated ADR 017](adr/017.md), [append validation](validation/APPLICATION_BUILDERS.md)
and [the broader measurements](../benchmarks/reports/2026-10-02/APPLICATION_BUILDERS.md).
Alias-heavy building and slices still copy. No views, ownership syntax or new
language profiles were added; general C++ parity remains an empirical question.

Scalar lazy regions now have a recursive nonallocation/nonescape proof in HIR
liveness. Their sequence captures remain semantically live, but duplicate physical
roots are unnecessary. Loop retention accepts these regions. CFG edges, lazy
selected-arm execution, traps, effects and tick counts remain unchanged. Unknown
calls/intrinsics, sequence results and nested loops were conservative in this first pass.
See [ADR 019](adr/019.md) and [the matched full-workload study](../benchmarks/reports/2026-10-03/LAZY_REGIONS.md).


The interprocedural borrowing summary now proves nonallocating, host-effect-free
scalar callees, including recursive groups, and nested identity-state loops.
Length/index/parse qualify; sequence-returning or allocating/effectful callees keep
roots. Semantic capture liveness, CFG, traps, aliases and resource checks are unchanged.
See [ADR 020](adr/020.md), [validation](validation/CALL_ROOTS.md) and
[the call-shape matrix](../benchmarks/reports/2026-10-03/CALL_ROOTS.md).


Allocation-free sequence-returning calls/regions now borrow aliases too; an allocating
caller roots returned values before collection. Slices still copy/allocate. A restricted
induction proof emits typed loads for identity-state sequences, constant nonnegative
starts, exact index < length conditions and +1 updates in allocation-free loops.
Facts end at body exit. Computed or mismatched ranges retain checks. Private header
layout/alignment is asserted in C; no noalias/nonempty-payload promise is assumed.
See [ADR 021](adr/021.md), [validation](validation/SEQUENCE_RETURNS.md) and
[measurements](../benchmarks/reports/2026-10-03/SEQUENCE_RETURNS.md).

## Application generation/repair measurement

The [frozen v5/Python study](../benchmarks/reports/2026-10-03/V5_GENERATION.md)
completed 48 trials / 122 attempts on four application tasks, three seeds and the
original pinned Gemma 3 4B / Qwen 2.5 7B Instruct digests. **V5 spent more total
input/output tokens and solved 0/24 trials; Python solved 11/24.** This is an
unfavourable measured result, not an unmeasured efficiency claim. Compiler,
runtime, profiles, task set, prompts and repair budgets were not tuned to improve it.

Failure-inclusive totals per model: Gemma v5 input 40,056 / output 9,855 versus
Python 17,496 / 5,996 (4/12 solves); Qwen v5 32,353 / 3,287 versus Python
11,251 / 3,009 (7/12 solves). Repair output is included, not added twice.
V5 TCR is undefined with zero solves; Python input-plus-output TCR is 5,873.0
and 2,037.1. No repair succeeded. The separate v5 source-density measurement below does not
count attempts or repairs; low failed-output counts would not establish savings.

All v5 attempts fail parsing; Python also fails semantic edge cases. The frozen
extractor's unsupported profile-labelled fence policy is a limitation. Removing
that label post hoc from 18 recorded attempts made none correct; this does not
create a counterfactual repair trajectory. Four authored task clusters, three
low-temperature repeats and two small quantized pretrained models cannot establish
a general representation winner or efficacy after different model adaptation.
All paired TCR ratio resamples are undefined. The current default remains unchanged.

The stopped numerical pilot remains incomplete and separate. An initial application
run was excluded for a sandbox Python loader defect; the corrected full run uses
mandatory interpreter-specific fixture preflight. See [the audit](experiments/V5_GENERATION_AUDIT.md),
[reproduction/oracles](../benchmarks/generation/application/README.md) and
[command evidence](validation/V5_GENERATION.md). No compiler bug was found or fixed.

## Verified v5 source density and adaptation seed

[The frozen corpus](../benchmarks/corpora/application-v5/README.md) has 25 application
tasks with rationales and executable oracles, 24 verified NIL references and 25 each
in Python/C++. Environment lookup remains unsupported: v5 cannot observe an
independently varying process environment. No task was dropped to improve results.
Five fixtures per task pass: 120 reference, 120 native O0, 120 native O2, 125 Python,
125 C++ checks. Independent literal goldens include binary/UTF-8 data and CR/LF
boundaries; finite tests are not exhaustive correctness proof.

**NIL uses 36.7–43.5% more raw source tokens than Python and 41.4–44.8% fewer than
C++, including headers and file I/O.** Measured aggregate NIL/Python/C++ counts:
Gemma 3196/2227/5454; Qwen 2418/1728/4235; cl100k 2338/1705/4217; o200k
2340/1712/4238. NIL is shorter in characters yet costs more tokens than Python.
Min/max and adjacent dedupe lose to both baselines under every tokenizer.
[Full sizes, losses, pins and methodology](../benchmarks/reports/2026-10-03/V5_DENSITY.md).
The exact generation GGUF vocabularies were loaded without inference; tokenizer
packages are confined to the benchmark Python environment. Compiler/runtime,
profiles/grammar/default and Rust dependencies remain unchanged; CI passes 208 tests.

**Source density is not model token efficiency.** The failure-inclusive generation
result remains unfavourable (0/24 versus 11/24); no syntax winner, successful
adaptation or general language ranking follows from this small authored corpus.
C++ framing favours NIL on small entry points; standard-library operations favour
Python. These are purposeful correlated tasks and implementation-specific counts.

Manual delimiter scans and counting passes, repeated parse expressions across
parallel loop updates, hand-written byte equality and missing buffer builders are
recorded generality costs. Environment access is a missing Host capability. Typed
operations, multiple-result bindings and builder ideas remain proposals; none was
implemented here. Python LF-only CSV reference review found and corrected a
`splitlines()` contract error; no compiler bug was found or fixed.

The MIT-licensed seed exports 24 hashed verified NIL positives. Four tasks overlap
the previous generation study; future adaptation needs family-separated holdouts.
24 examples support few-shot seeds, not a usable fine-tuning claim. A heuristic
500–2,000 independent verified-task pilot plus repair/negative trajectories would
be roughly 20–80x larger; no model-specific data requirement or success is proven.
No inference or adaptation run was conducted for this measurement.
