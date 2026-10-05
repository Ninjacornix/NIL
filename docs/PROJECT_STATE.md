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
supports lines-v0, expr-v1, expr-v2 and opt-in expr-v3/v4/v5. Frontends resolve/type-check all functions
and validate syntax-independent HIR. The CLI supports check/hir/llvm/build/run;
run/build use LLVM natively, while the library retains a bounded reference evaluator.
M3 adds explicitly typed bool/array functions and immutable fixed-size i64 arrays
in expr-v4; expr-v5 adds binary64, u64 and u128 under ADR 027 and bounded
nominal records under ADR 028. Other
widths and explicit memory remain deferred. M4 canonical
serialization is absent; a local M6 generation/repair runner exists; its pilot was interrupted.
M5 source measurement and M7 profile experiments are implemented to useful extents;
M10 native execution is delivered for the current core. Version 1 local, compiler-visible borrowing plugins are prototyped; a stable
foreign ABI and owned/effectful providers remain unimplemented;
model adaptation and self-hosting remain conditional investigations. The
[milestone audit](MILESTONE_AUDIT.md) distinguishes delivered capabilities from
remaining original-plan requirements.
The expr-v0 default was chosen after the local paired benchmark showed lower source-token counts for three arithmetic programs on two pinned tokenizers, while execution lowered to identical HIR. This is partial M7 work; model-generation trajectories and broader syntax selection are still open.

## Working assumptions and architecture

Current pipeline: text → syntax AST → signature resolution/type checking/lowering →
validated HIR → backend SSA CFG → LLVM → native executable. Four compiler crates separate
semantic IR, compiler/frontend/reference execution, native backend and CLI; a fifth
workspace package provides seeded fuzz testing. HIR has
typed immutable values, typed calls, explicit v5 host effects, structured regions
and one return per function.
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

## Fixed verified-example exposure (2026-10-05)

**The fixed few-shot study changed failure modes: NIL solved 0/24
control and 0/24 with four verified examples; rerun Python solved
11/24.** Parsing passed 0/72 control attempts versus
53/72 few-shot; 18 few-shot attempts typechecked
and 18 compiled. Static-stage success is not task correctness: the report separates
wrong-task example copies, type errors and semantic/execution failures. Input/output
and repair totals are charged in full; zero-solve TCR is undefined. This is four
tasks, three seeds, two small quantized models and one rule, not evidence that
fine-tuning works or that NIL saves total model tokens.

All 18 compiled few-shot candidates exactly copy count_newlines for the wrong
tasks. Across both models, control input/output **72,409 / 13,142** versus few-shot
**106,209 / 10,892** means **36.9% more total tokens**, still no correct NIL program.
These pooled counts are descriptive; model-specific costs are below.

This supports a narrow exposure/imitation effect on accepted syntax, not correctness or model-efficiency improvement. Exact example copying does not demonstrate grammar generalization to a new task. The stronger claim that familiarity explains zero solves, or that fine-tuning fixes them, remains untested.

The frozen control already contained a generic copy-file example. The treatment
retained that reference and added globally fixed invert_bytes, histogram_digits,
count_newlines and hex_encode sources. No compiler, corpus, task, oracle, budget,
feedback or extraction policy changed. Selection committed before inference; no
smoke trials, prompt tuning or automatic retries. Both exact Gemma/Qwen digests
ran control and treatment in one session; Python reran as the common baseline.

gemma3:4b:

- control / expr-v5: 0/12 solves; input **40,056**, output **9,855**, repair output **6,570**; total TCR **undefined**.
- control / python: 4/12 solves; input **17,496**, output **5,996**, repair output **3,610**; total TCR **5873.0**.
- fewshot / expr-v5: 0/12 solves; input **52,047**, output **2,286**, repair output **1,524**; total TCR **undefined**.

qwen2.5:7b-instruct:

- control / expr-v5: 0/12 solves; input **32,353**, output **3,287**, repair output **2,193**; total TCR **undefined**.
- control / python: 7/12 solves; input **11,251**, output **3,009**, repair output **1,494**; total TCR **2037.1**.
- fewshot / expr-v5: 0/12 solves; input **54,162**, output **8,606**, repair output **5,592**; total TCR **undefined**.

Presentation-policy limitations still apply: complete expr-v5-labelled fences
fail extraction. Exact example copies can parse without transferring to the new
task. Serial condition order, related authored examples and the four-task sample
limit causal inference. No model inference beyond this frozen run, training or
language changes followed. CI: **368 tests, zero warnings**. All commits local;
nothing pushed. [Full per-task/seed usage and failure modes](../benchmarks/reports/2026-10-05/V5_FEWSHOT.md),
[preregistration](experiments/V5_FEWSHOT_PLAN.md),
[commands](validation/V5_FEWSHOT.md).

## Ordered keyed application data — before algorithm operations

[ADR 023](adr/023.md) selects maps before records from the frozen external corpus.
V5 now has immutable insertion-ordered Bytes->I64 and Bytes->Bytes maps, with
typed construction, membership, lookup, insert/update, size and key iteration.
Keys/byte values are owned; no nested map values or fields. Runtime last-use plus
single-root checks permit reuse; aliases copy. Charges include geometric entry,
hash-table and byte capacity under the unchanged 64 MiB live-capacity quota.
Missing keys use E019 and duplicates E020 before result quota; invalid iteration
uses E012, invalid types E007, absent dot syntax E001. Earlier profiles/default
and Rust workspace dependencies are unchanged.

**The Python density gap remains: 12.0–21.1% more externally and 6.9–14.6% more
combined**, across Gemma, Qwen, cl100k and o200k. The original 24 counts are unchanged.
Grade-school duplicate tracking saves 10–11% on that task; newly supported ETL is
still more than twice Python's source-token cost. Its addition worsens the expanded
aggregate despite a modest reduction on the same previously supported tasks.
[All measurements](../benchmarks/reports/2026-10-03/V5_KEYED_DATA.md) distinguish
cohort change from operation impact. Source density is not model efficiency;
the unfavourable generation result remains 0/24 versus 11/24, with no new inference.

75 tasks remain, with 56 verified NIL references and 19 unsupported (18 external).
ETL moves to supported; maps do not themselves supply the Forth interpreter,
alphametic search, game simulation, graph parser or a typed owner/coordinates
product. Contracts, oracles, goldens and existing baseline sources remain unchanged;
support is a metadata overlay. 2,400 checks pass, original 610 separately confirmed.
[Validation](validation/V5_KEYED_DATA.md) records O0/O2, sanitizer, seeded fuzz and
performance evidence. Byte-value replacement of a different length repacks the
owned payload and is explicitly not universally constant-time.

## Algorithmic operations — density snapshot before numeric/record expansion

[ADR 025](adr/025.md) preregistered sorting and snapshot iteration before compiler
implementation. `sort` accepts dynamic sequences/maps with specified signed,
unsigned-byte or value-then-key ordering; guarded last-use reuse preserves aliases.
`each` lowers to canonical existing HIR loops, binding index/element or key/value
and explicit parallel user state. Input snapshots stay immutable. Owned map keys
and byte values still allocate, even when ignored; quota and effects stay observable.
Earlier profiles/default, 64 MiB and the 262145-entry insertion ceiling are unchanged.

**External NIL still costs 4.3–12.3% more source tokens than Python; combined,
1.2–8.1% more.** It costs 46.4–49.7% less than C++ externally. Eleven external
references improve and 21 do not; original 24 references and every baseline,
contract/oracle/golden remain unchanged. Sort saves 205 cl100k tokens; each saves
193 more after sort. Joint 398/650 (61.2%) matches the preregistered forecast,
leaving a 252-token external gap. Numeric literal surplus falls only 23 tokens;
separator/index scaffolding accounts for most movement. [All four tokenizers,
three cuts and exact attribution](../benchmarks/reports/2026-10-03/V5_ALGORITHMS.md).

75 tasks remain: 56 verified NIL programs/exports, 19 unsupported (18 external).
Neither operation supplies missing Unicode, widths, floats, nested types or
unimplemented search/interpreter/graph adapters. Source density is **not** model
efficiency; the generation result remains NIL 0/24 versus Python 11/24. No inference.

256 debug/release tests and 69 ASan/UBSan tests pass. Three seeds compare 1008
programs/2016 native O0/O2 builds with zero divergences; all 2400 corpus checks and
original 610 pass. [Validation receipt](validation/V5_ALGORITHMS.md). Performance
controls measure scan 7.64 ms (16 MiB), append 26.10 ms (1 MiB), transform 38.25 ms
(16 MiB); append's median is 8.7% higher than the previous run, not hidden. Old
controls' executable IR is unchanged apart from unused sort declarations. Native
sort scales O(n log n); map each retains owned-byte costs, not free borrowed views.

The attribution and keyed-data sections below describe earlier frozen rounds.

## External token attribution — before algorithm operations

[ADR 024](adr/024.md) retains the current syntax after a complete token ledger
on all 32 supported external pairs. cl100k remains 5686 NIL versus 5036 Python
(+650, +12.91%). The rational byte-overlap ledger ranks numeric literals (+394),
operators (+325.738), separators (+320.513) and signatures (+318) as positive
costs, offset by NIL's whitespace/keyword/identifier savings. Every token is
accounted once; mixed-token attribution sensitivity is reported separately.
Grade School, Connect and ETL contribute +523 of the net gap, exposing explicit
sorting/comparison, coordinate traversal, and format-processing costs.

Whole-source counterfactuals save only 32 cl100k tokens for repeated signature
references and nine for empty unchanged-state steps. Both remain invalid syntax;
they were not added to the compiler or corpus. Actual implemented saving is zero.
The combined overseer textual-repeat ceiling does not justify local bindings as
an answer. No existing program's meaning, source or oracle changed.

[The preregistered analysis](../benchmarks/reports/2026-10-03/V5_ATTRIBUTION_PLAN.md)
recommends a separate semantic-abstraction decision with verified reference
rewrites before predicting larger savings. This is a limitation of current
solutions, not proof that immutable values must change or that the gap is
unclosable. Raw density remains separate from the unfavourable 0/24 versus 11/24
generation result. No model inference occurred.
[Validation receipts](validation/V5_ATTRIBUTION.md) record 239 debug/release tests,
55 sanitizer tests, three zero-divergence seeds, all 2400 corpus checks and fresh
performance controls.

## External v5 density and adaptation seed (before keyed maps)

**The Python density advantage does not hold on the expanded external sample.**
NIL uses **10.6–19.9% more source tokens than Python** on 31 supported external
properties and **5.8–13.6% more** on the combined 55 paired tasks. It uses 42–46%
fewer than C++ externally and 48–51% fewer combined, including headers/I/O.
[Three-cut tables and full methodology](../benchmarks/reports/2026-10-03/V5_EXTERNAL_DENSITY.md).
The original 24 still reproduce their favourable counts exactly, as recorded below.

The corpus retains 75 tasks: 25 self-authored and 50 externally derived from the
first alphabetical canonical-data exercises in pinned MIT-licensed Exercism
specifications. Each new task targets its first declared property, not its whole
upstream API. Selection and transport were committed before implementations/counts.
**Twenty tasks have no verified NIL solution: 19 external plus environment lookup.**
Gaps include floats, wide integers, Unicode, entropy, concurrency/reactive contracts
and missing map/tree/dictionary/calendar/multi-file adapters. Some could be emulated;
unsupported is not uniformly a theoretical impossibility claim. No task was dropped.
[Per-gap counts](../benchmarks/corpora/application-v5/GENERALITY_GAPS.json).

55 NIL programs pass in-memory reference and native O0/O2, with supported Python/C++
references: 2,380 execution checks. The original 610 separately still pass; original
tasks/oracles/goldens and all 74 sources are byte-identical. CI passes 220 tests.
Compiler/runtime/grammar/profiles/default, Rust dependencies and execution limits
are unchanged. No production defect was found; two reference algorithms were
replaced after exhausting the unchanged fuel budget. Armstrong's 108/127-bit cases
were retained and classified unsupported; invalid binary wording was corrected
before counting without changing frozen expected values.

Fifty external exercise names add 31 verified positives, for 55 exported examples.
This is a larger seed, not adaptation scale: roughly 9–36× below the earlier
heuristic 500–2,000-task pilot, with strong family correlation and finite fixture
oracles. Alphabetical/curriculum ordering, API slices, byte transports, local authors
on both sides and unsupported exclusions prevent a representative population claim.
**Density is not total model work; generation remains unfavourable, 0/24 vs 11/24.**
No inference or training occurred. All commits remain local; nothing pushed.

### Original 24-task evidence (historical cohort)

[The original cohort](../benchmarks/corpora/application-v5/README.md) has 25 application
tasks with rationales and executable oracles, 24 verified NIL references and 25 each
in Python/C++. Environment lookup remains unsupported: v5 cannot observe an
independently varying process environment. No task was dropped to improve results.
Five fixtures per task pass: 120 reference, 120 native O0, 120 native O2, 125 Python,
125 C++ checks. Independent literal goldens include binary/UTF-8 data and CR/LF
boundaries; finite tests are not exhaustive correctness proof.

The initial density study measured 36.7–43.5% more NIL tokens than Python.
The corpus-driven generality additions now measure **3.7–8.2% fewer than Python
and 60.7–62.9% fewer than C++**, including headers and file I/O. Aggregate
NIL/Python/C++ counts: Gemma 2144/2227/5454; Qwen 1634/1728/4235; cl100k
1582/1705/4217; o200k 1571/1712/4238. Eleven references improved under all four
tokenizers; thirteen are unchanged. Six to eight tasks still lose to Python,
depending on tokenizer. [Full before/after results](../benchmarks/reports/2026-10-03/V5_GENERALITY.md).

[Pre-implementation ranking](experiments/V5_GENERALITY_IMPACT.md) prioritized
bulk canonical integer parsing, delimiter search and sequence equality. Their
verified counts exactly match the explicitly unverified forecasts. HIR signatures,
reference evaluation, native execution and borrowing summaries implement `parsebuf`,
`find` and `equal`; [ADR 022](adr/022.md) records allocation/error ordering.
Tasks, oracles, goldens and baseline sources are unchanged; 610 checks still pass.
CI passes 220 tests, ASan/UBSan 36 native application tests, and three seeds compare
936 programs against 1872 native O0/O2 builds without divergences. Earlier profiles,
grammar/default and Rust dependencies remain unchanged.

**Source density is not model token efficiency.** The failure-inclusive generation
result remains unfavourable (0/24 versus 11/24); no syntax winner, successful
adaptation or general language ranking follows from this small authored corpus.
C++ framing favours NIL on small entry points; standard-library operations favour
Python. These are purposeful correlated tasks and implementation-specific counts.

Bulk parsing removes numeric scan/count scaffolding, search exposes line boundaries,
and equality removes a hand-written comparator. Remaining costs include repeated
slices in deduplication, hex/CRLF loops, positional loop state and lack of multi-result
bindings/records. `parsebuf` allocates fresh storage and charges input, result and
caller live capacities; it is not a view. Builders and environment access remain
open. No syntax compression or model inference was performed. No production defect
was found by this goal's fuzz/sanitizer campaigns; see [validation](validation/V5_GENERALITY.md).

The MIT-licensed seed exports 24 hashed verified NIL positives. Four tasks overlap
the previous generation study; future adaptation needs family-separated holdouts.
24 examples support few-shot seeds, not a usable fine-tuning claim. A heuristic
500–2,000 independent verified-task pilot plus repair/negative trajectories would
be roughly 20–80x larger; no model-specific data requirement or success is proven.
No inference or adaptation run was conducted for this measurement.


## Numeric core and extension boundary (ADRs 026–027)

[ADR 026](adr/026.md) makes proof participation a compiler-visibility requirement,
not a license for permanent intrinsic growth. Fundamental typed value/control/
storage contracts stay core; algorithms may move to validated compiler-visible
extensions only with equivalent lifetime/alias/reuse/quota/trap/effect contracts.
Every pre-existing intrinsic is classified; none migrates. ABI remains deferred
until aggregate marshalling and the type/effect/lifetime ABI contract are designed.

V5 adds only unsigned u64/u128 and binary64 f64 scalars, explicitly converted and
without implicit mixed-width arithmetic. IEEE primitive operations are strict and
unfused, all NaNs canonical, signed zero observable through bits/format; float
zero division produces IEEE infinity/NaN while integer zero division traps E009.
Checked conversions fail E021 before casts; unsigned/float parsing fails E022.
Fixed scientific 17-digit formatting round-trips canonical bits. Numeric maps,
collections, other widths and transcendental operations remain absent.

The unchanged frozen contracts now verify Armstrong Numbers, Grains, Darts and
the selected Complex Numbers real-part property: **60 verified NIL programs,
15 unsupported tasks (14 external)**. The original 24 solutions/baselines/counts
are unchanged. External source density still loses to Python by **4.2–12.4%**;
combined it loses by **1.2–8.4%**, across four tokenizers. NIL uses 47–50% fewer tokens than C++ externally.
These are cohort-expansion results, not savings on the existing 56 tasks.
[Numeric evidence and full tables](../benchmarks/reports/2026-10-04/V5_NUMERICS.md).
Source density does not establish failure-inclusive model efficiency; the earlier
0/24 NIL versus 11/24 Python generation result remains unfavourable.

## Records — current delivery and density

[ADR 028](adr/028.md) was committed before implementation and applies ADR 026 at
operation-selection time. Named immutable records are inline typed HIR/LLVM
products; nested records and sequence/map fields retain each dynamic leaf's roots
and live-capacity charge. Construction, projection and functional update are core
product/lifetime primitives; typed scalar-record map construction is core storage.
There are four new HIR operations, **zero new Intrinsic variants (32 before/after)**.
Equality/order, reflection, serialization, formatting, defaults and bulk transforms
were rejected as future library surface. Destructuring and shortest float formatting
remain deferred; no plugin ABI or intrinsic migration was implemented.

Scalar/fixed-array records can be map values. Sequence-bearing record map values,
record buffers, recursive types and native record entry marshalling are deferred;
use scalar/sequence entry wrappers. Product updates allocate no arena storage.
Whole-record liveness stays conservative: sequence-field appends can still copy,
measured at 3.653 / 16.955 / 396.285 ms for 10k / 40k / 160k iterations. No reuse
proof was weakened. Scalar-record map insertion reuses proven unique storage;
Pair(i,i)'s tested workload ceiling is 262,144 entries, then E013. Existing integer
map ceiling remains 262,145. The 64 MiB quota and earlier/default profiles stay fixed.

**The external Python density gap widens to 12.9–23.3%, and combined to 8.0–16.8%.**
NIL remains 42–46% below C++ externally and 47–51% below combined across Gemma,
Qwen, cl100k and o200k. The old 60 solutions/counts are unchanged; newly supported
Go counting adds a costly fixed-point flood-fill/JSON adapter. This is cohort
movement, not a fixed-task record saving. The preregistered zero automatic savings
forecast was met. Go loses to Python and C++ under every tokenizer. The cl100k
external attribution sums 260 → 767 (+507), entirely the new task; record declarations
account for 30, while algorithms/signatures/separators contribute much more.
**Density does not establish model efficiency:** the generation result remains
NIL 0/24 versus Python 11/24, including failed attempts and repairs.

There are **61 verified NIL programs and JSONL positives; 14 unsupported tasks
(13 external)**. Go counting is one delivered unblock of five candidates.
Alphametics still lacks a verified search/JSON adapter; dot-dsl lacks structured
collections/sequence-bearing record maps; Camicia's decks fit records but its
verified game/cycle adapter is absent; binary-search-tree still needs recursive
children or a verified encoding. All remaining blockers are retained in the
[gap catalogue](../benchmarks/corpora/application-v5/GENERALITY_GAPS.json).
The nonrandom partial-API corpus is still far below credible adaptation scale.

[Validation](validation/V5_RECORDS.md): 311 tests in debug/release, no warnings;
105 native sanitizer tests; three seeded campaigns compare 720 programs at O0/O2
(1,440 builds), zero divergences. Frozen corpus checks: 2,625, including the
separately confirmed original 610. Tasks/oracles/goldens/existing baselines remain
unchanged. Twenty-five-repeat controls: scan16MiB 7.573 ms, append1MiB 24.626 ms,
transform16MiB 34.991 ms; no clear regression established under nonisolated timing.
[Report, density and attribution](../benchmarks/reports/2026-10-04/V5_RECORDS.md)
retain every loss, raw samples, quota/reuse boundaries and implementation findings.

## Validated semantic plugin prototype (ADR 029)

Local `nil-plugin 1` manifests link typed NIL providers; **no native/dylib ABI or
sandbox**. The compiler derives nonallocating/non-host borrowing from validated
bodies, preserves caller arena identities, aliases and quotas, and rejects unproved
contracts. All current types pass within existing collection limits. Allocating,
host-effect, recursive/nested plugin providers remain unsupported.

Equality moved to the shipped provider with its old alias and diagnostics preserved;
one plugin-only record update demonstrates authoring. Intrinsic enum stays 32
(31 executable core implementations plus compatibility adapter). No corpus source or
model efficiency result changed. [Authoring](architecture/PLUGINS.md).

[Validation](validation/V5_PLUGINS.md): 331 debug/release tests, no warnings; 116
sanitizer tests; 792 programs/1,584 O0/O2 builds over three seeds, zero divergences;
2,625 unchanged corpus checks plus original 610 separately. Twenty-five-repeat
controls: scan16MiB 7.581 ms, append1MiB 24.616 ms, transform16MiB 35.357 ms.

[Boundary measurements](../benchmarks/reports/2026-10-04/V5_PLUGINS.md): repeated
16 MiB file hot calls improve at O2 (433.580 → 396.647 ms), add 4.185 ns/call at O0,
with borrowing/reuse proofs intact. **Bulk equality worsens 7.232 → 16.859 ms at O2**,
13.12x at O0: portable element loops replace optimized `memcmp`. This is a real
algorithm regression. Recommend an optimized compiler-visible lowering before
production adoption; foreign ABI costs remain unmeasured. Source-linking soundness
is demonstrated, not a complete library ecosystem or universal fast plugin boundary.

## Acyclic structured collections (ADR 030)

`v[Record]` and typed `!buffer[Record](length,fill)` add dynamic homogeneous
record collections; records can contain earlier record buffers, ordinary sequences
and maps. Integer-index node links and wrapper records enable trees/lists/graphs
without recursive types or cyclic ownership. Constructor/index/replacement/concat/
slice/each share the existing validation, alias, quota and reuse contracts. Intrinsic
variants remain **32**. Sequence-bearing record map values, generic sequence/map
entries and record sorting remain deferred, with explicit runtime sort E018 rather
than invented ordering. Plugins require an explicit record-buffer type capability.

Quota is unchanged at 64 MiB live capacity. Packed rows charge 40+(capacity+1)*W;
transitive child allocations are charged once by identity. Static last-use and
actual unique roots permit reuse; outer child-edge roots prevent false uniqueness.
Scalar-record storage grows geometrically; dynamic-child root transitions can
traverse all rows, so nested builders/updates have a conservative performance boundary.
Record-field append remains superlinear. No old-profile/default behavior changed.

Four pinned MIT algorithm tasks and the frozen external BST are verified. **79
tasks, 66 NIL programs/export positives, 13 unsupported (12 external)**. Camicia,
Forth and Alphametics still lack verified adapters; Dot DSL also lacks its adapter
and generic keyed nesting. Remaining blockers are in the gap catalogue, separating
missing types from missing implementations. Frozen canonical BST strings do not
establish a complete JSON/Unicode parser.

**Source density worsens with the harder cohort: external +21.9–31.3% versus
Python, combined +15.6–23.7%; the new four alone +66.6–67.6%.** The old 61 program
sources and counts are unchanged; new BST/algorithms explain the movement. NIL
remains below C++ in aggregate, but the new four lose under three vocabularies.
Density is not model efficiency; generation remains NIL 0/24 versus Python 11/24.
The corpus remains small, nonrandom partial-API seed data, far from adaptation scale.
See [the full report](../benchmarks/reports/2026-10-04/V5_COLLECTIONS.md) and
[validation](validation/V5_COLLECTIONS.md) for commands, raw samples and boundaries.
Bulk equality's plugin regression remains outstanding and was not changed here.

Final gates: **358 debug/release tests**, no warnings; **135 sanitizer tests**;
**924 differential programs / 1,848 native builds** over three seeds, zero divergences.
Frozen corpus **2,775 checks**, original **610 separately**, random algorithms **600**,
type-layout feasibility probes **12**. Intrinsic variants remain 32.

Native scalar-record build1Mi rows is 56.66 ms, update 12.88 ms; retained-child
update 44.36 ms. Nested fresh/shared builders remain quadratic (16k: 1,663.41 /
1,008.03 ms). Aliased builders copy. Record-field append is still superlinear
(10k/40k/160k: 3.61/16.95/409.22 ms), up from the earlier 322.69 ms at 160k.
Its paired controls are 337.37→402.06 ms against immediate pre-fix collections,
but 381.62→379.30 ms against historical runtime; both are retained. That
revision-dependent field-builder cost is not isolated further or labelled noise.
Field-sensitive reuse, nested root/arena scaling, keyed payload repacking and
reference-evaluator speed remain deferred. No claim of general C++ parity.

**The ordinary byte-append regression is repaired:** pre-fix/current paired control
39.158→24.540 ms; historical/current 24.463→24.545 ms. A validated whole-program
scan covers signatures, instructions, lazy/loop regions and plugin providers,
following reachable record fields. Only programs with no record buffers disable
transitive children and tagged-size quota logic. Record-buffer/mixed programs
remain conservative. No bounds, quota, alias, trap/effect check was weakened.
Standard controls: append1MiB 24.764 ms, scan16MiB 7.836 ms, transform16MiB
37.922 ms. Transform is inside the historical 34.77–41.24 ms range; the runtime
A/B 40.289→39.878 ms does not attribute the overseer's elevation to unavoidable
child tracking. Timing is not CPU-isolated and not a claim to explain every
cross-session variation. The known bulk equality ~16.96 ms regression is unchanged.

**A real zero-slot layout defect was found after the initial clean campaigns.**
`record Z(x:0)` buffers divided by zero and crashed at O2 while O0 returned a
value. UBSan confirmed the division; zero metadata width also failed to reserve
pointer storage. Physical row width is now 8*max(1, logical slots), with an
initialized padding word and matching reference/native live-capacity charges.
Logical fields and zero-byte scalar-record map encoding are unchanged. No valid
empty record type was rejected. Twenty new degenerate families cover single/all-zero,
mixed and nested products in buffers/maps/loop state, aliases, laziness and traps.
The exact overseer source returns 2 in reference, O0/O2 and sanitized O0/O2.
All final gates above were rerun after the last code change; the original
353/131/864 campaigns missed this class and are superseded completion evidence.


## Performance debts resolved and retained (ADR 031, 2026-10-05)

This supersedes the collections delivery's outstanding performance-debt list;
historical numbers above remain recorded as history. No type, syntax, intrinsic,
profile, corpus contract or density result changed this round.

Bulk equality's provider regression is repaired by a proof over the complete
validated HIR scan, independent of provider name/ID: **16.97 → 7.22 ms**
at O2 on 16 MiB (historical pre-plugin 7.23 ms). Third-party providers benefit;
unproved near-misses keep normal checked lowering and body-derived effect proofs.

Direct record-field append is now approximately linear: 160k **396.65 → 7.57
ms**. Frozen identical-IR controls with only the record-buffer runtime macro changed
isolate the old comparison-dependent slowdown to root-call inlining/code layout,
with identical copy counts and zero child traversal. Exact CPU-cycle attribution
was not measured. Transferring only a provably dead field edge removes the copies,
while actual caller/other-field/parent aliases still prevent reuse.

Single-use fresh/shared nested builders are approximately linear in the measured
range: 16k **1,663 ms → 4.59 ms** fresh and **1,008 ms → 3.96
ms** shared. Retained concat roots avoid child-edge backedge walks; a live-relative
garbage budget bounds sweep work. The first physical-exhaustion-only policy caused
real keyed/nested-copy regressions; bounded collection corrected them. Applying
that policy to flat programs also slowed append, so validated record-buffer-free
programs keep their established eager path. All gates were repeated afterward.

Controls: append1MiB **22.85 ms** (historical 24.76), scan16MiB **7.60
ms** (7.84), transform16MiB **36.67 ms** (37.92). Remaining limits:
child-field append through a live parent row and genuine alias builders copy and
remain superlinear; size-changing keyed payload repacking, allocating nested
regions/calls outside the proof and reference live-graph traversal remain
conservative. The bulk equality debt and direct-builder debts are no longer on
the accepted-limit list. No universal backend/plugin performance claim follows.

Final validation: **368 debug/release tests**, zero warnings; **142 sanitizer tests**;
three seeds, **996 programs / 1,992 native builds**, zero divergences; **2,775 corpus
checks**, original **610 separately**. Frozen source hashes are unchanged.
All commits are local, nothing pushed. See [the measured curves and receipts](../benchmarks/reports/2026-10-05/V5_PERFORMANCE_DEBTS.md)
and [proof notes](validation/V5_PERFORMANCE_DEBTS.md).


## Local source modules (ADR 032)

The existing plugin loader now accepts `nil-module 1` manifests with explicit
numeric exports and relative transitive imports. It resolves imported calls into
ordinary HIR Call before validation, so normal caller allocation, effects,
recursion, roots and interprocedural borrowing proofs apply. Restricted
`nil-plugin 1` semantics remain unchanged. No runtime, grammar, type or intrinsic
was added: enum count stays 32. Root positional labels and entry 0 are unchanged;
imported private labels cannot be selected as CLI entries. Import cycles are
rejected and nominal record declarations remain in the root registry.

Naming was measured before implementation: qualifying all 328 local calls in the
66 unchanged solutions adds 1,418–1,666 tokens across four tokenizers. Therefore
only imported references are qualified. The two real split solutions (newline
count and integer sum) share a reporting library and preserve frozen goldens;
the complete bundle costs 33–39 extra tokens. Sharing a tiny formatter is not a
density win. All 66 single-file source bytes and counts remain unchanged.

`multi_file_fixture` means upstream grep's runtime text-file fixtures, not source
imports. Modules do not supply that missing host adapter; the 13 unsupported tasks
remain unsupported. No corpus contracts, baselines or goldens were edited. No
model inference, commutative bulk matcher repair or further capability followed.
Final gates: 385 tests in debug and release, no warnings; 150 sanitizer tests,
1068 differential programs/2136 native builds with zero divergences; full 2775
corpus checks, original 610 separately, plus 30 split checks. Controls: append
22.899 ms at 1 MiB, scan 7.728 ms and transform 36.819 ms at 16 MiB, inside the
preceding sample ranges with identical single-file IR. Commands and receipts
are recorded in validation/V5_MODULES.md.

## Host capabilities and remaining-gap placement (ADR 033)

Environment lookup, seedable entropy-state draws and sorted directory snapshots
are core host effects. They cannot be invented by source plugins; algorithms on
top remain compiler-visible library/module work. Modules share the caller's Host;
borrow-only providers reject the effects. Reference execution still denies all
three without an explicitly supplied Host. Intrinsic variants: **32 -> 35**;
three additions, each an authority/effect/owned-result contract, no new types.

Randomness uses injectable SplitMix64 state with exact u64 parity; native injection
is `NIL_RANDOM_SEED`, reference injection is `SeededHost`. Production opt-in FileHost
seeds lazily from `/dev/urandom`; it is not cryptographic randomness. There is no
unchecked/non-oracle operation. Independent OS seeds are never compared as if equal.
Directory order is unsigned-byte lexicographic; snapshots use the existing map
layout, live-capacity quota and root tracking. This macOS filesystem rejected the attempted `0xff`
filename fixture; injected raw-name and native non-UTF-8 environment tests are
separate evidence, not a claim that the filesystem permits arbitrary byte names.

**One corpus task newly verified, not the target three:** env_lookup. All frozen
contracts and baseline sources remain unchanged. Diffie–Hellman still has a
symbolic predicate golden and no concrete p/key correctness oracle; emitting that
predicate would not prove key generation. Grep poem fixtures are now checked in,
but fixture mounting plus a JSON/flag adapter are still needed for a verified task.
No plugin-side algorithm was added to claim coverage. Corpus: **67 verified NIL
programs/export positives, 12 unsupported**, 2,790 checks. The old original 610
checks are a subset that retains environment's unchanged Python/C++ checks.

Placement of the remaining tasks is recorded individually in ADR 033 and
GENERALITY_GAPS.json. Higher-order callbacks and concurrency are core execution/
abstraction decisions; defer concurrency explicitly. Reactive integration needs
a callback contract, not a sequential transcript. Unicode, calendar, graph/deck/
constraint-search/dictionary adapters and grep processing belong to the library
backlog. Historical gap names do not imply these encoded algorithms are
inexpressible with current collections.

Density with environment newly paired: original 25 **6.2–10.6% fewer tokens than
Python**, external 42 **21.9–31.3% more**, combined 67 **15.0–22.9% more** across
Gemma, Qwen, cl100k and o200k. The old 66 source solutions/counts are unchanged;
this is a cohort addition, not a syntax improvement. NIL remains 43.1–46.3% below
C++ combined. Density does not establish model efficiency; the unsuccessful
few-shot generation finding remains unchanged.

Validation and measured controls: [host effects receipt](validation/V5_HOST_EFFECTS.md).
