# Milestone audit — 2026-09-30

Audited `master` at `c023b4e` against all 13 milestone plans, compiler modules,
tests, ADRs, benchmark runners/reports, and CI. Completion below describes supported
capabilities, not a production-readiness claim. Historical delivery records remain
historical; test counts in old reports are not current acceptance gates.

## Findings

| Milestone | Implemented evidence | Remaining work / disposition |
|---|---|---|
| M0: research | Original reports, project state, ADRs, architecture, measurement protocol. | Complete for initial consolidation; external research claims remain unverified. |
| M1: minimal execution | Functions, i64 arithmetic, calls, signatures, diagnostics, evaluator, CLI and compile-fail tests. | Complete. Native execution now also exists. |
| M2: control flow | Bool intermediates, six comparisons, lazy branches, typed state-tuple loops, nested loops and scope validation. | Complete; no separate for/while constructs needed. |
| M3: types | Strong static i64/bool HIR checking already exists; source function signatures remain i64. | Broader types are unimplemented. Choose acceptance programs before arrays, floats, references or extra widths. |
| M4: canonical IR | Syntax-independent immutable HIR, documented invariants, independent validation, deterministic lowering and cross-profile HIR tests; internal LLVM SSA/phi lowering. | Partial. Versioned canonical serialization, round trips and a separately validated portable MIR are absent. A duplicate MIR is conditional, not required for the existing backend. |
| M5: token benchmarks | `benchmarks/paired/run.py`, experiment/control/native/C++ runners, two pinned tokenizers, hashes, Unicode/byte counts, manifests, JSON reports and tests. | Core source-measurement capability delivered. Full plan remains partial: unified attempt JSONL, C/Rust baselines and canonical-IR corpus absent. Reuse runners rather than requiring a new tool directory. |
| M6: model generation | Failure-inclusive TCR/TTCP and experimental protocol documented. | Runner, model adapter, hidden-test protocol, repair trajectories and statistical analysis unimplemented. Fuzz-generated programs are not model-generated evidence. |
| M7: syntax | lines-v0 and expr-v0/v1/v2/v3, explicit selection, byte-span diagnostics, equivalence tests and source-token studies. | Partial. No controlled model/repair comparison, constrained decoding or final winner. Prefix/numeric/structured candidates are optional hypotheses, not compulsory features. |
| M8: plugins | Extension seam described in architecture; no plugin operation in HIR or runtime. | Unimplemented. Retain as a future goal; start with primitive types and one test operation when selected. |
| M9: framework | Domain experiment plan only. | Deferred until M8 and a baseline experiment. HTTP is a candidate, not a required web-stack commitment. |
| M10: native code | nil-llvm, Clang AOT build/link, native CLI, O0/O2 differential tests, separate build/runtime/size measurements and macOS release packages. | Delivered for the current core. Production hardening remains separate. Additional backends, aggregates and plugins cannot block completion of already supported native operations. |
| M11: model adaptation | Plan only. | Deferred investigation. Grammar constraints may be tested with M7; custom tokens/tokenizer/training require measured benefit and explicit budget. |
| M12: self-hosting | Plan only. | Optional late investigation, not a language-success requirement. |

[Fuzz infrastructure](FUZZING.md) is also delivered: seeded source/HIR mutations,
a typed-tree oracle, native differential workers, deadlines, replay, corpus and
nightly campaigns. Coverage-guided fuzzing and automated shrinking remain optional
improvements. It does not measure model success or prove absence of bugs.

## Evidence map

- [HIR definitions](../crates/nil-hir/src/lib.rs), [validator tests](../crates/nil-hir/tests/validation.rs),
  [region tests](../crates/nil-hir/tests/control_flow.rs), and [architecture invariants](architecture/OVERVIEW.md).
- [Profiles and equivalent HIR](../crates/nil-compiler/tests/expr_compact.rs),
  [legacy compatibility](../crates/nil-compiler/tests/expr_profile.rs), and [v3 arithmetic tests](../crates/nil-compiler/tests/expr_v3.rs).
- [Token runner](../benchmarks/paired/run.py), [runner tests](../benchmarks/paired/tests/test_runner.py),
  [measurement protocol](../benchmarks/README.md), and [recorded reports](../benchmarks/paired/results/2026-09-30/README.md).
- [LLVM lowering](../crates/nil-llvm/src/emit.rs), [native differential tests](../crates/nil-llvm/tests/native.rs),
  [native measurements](../benchmarks/paired/results/2026-09-30/NATIVE_LLVM.md), and [release packaging](RELEASES.md).

## Plan corrections

1. Reuse the current benchmark infrastructure. A `tools/tokenbench` executable is a
   packaging choice; it is not the missing evidence. Extend provenance, held-out
   manifests and attempt logging where needed.
2. A current-core M6 pilot needs fixed semantics, equivalent tasks, hidden tests,
   usage accounting and safe execution. Arrays, canonical serialized HIR and a
   portable MIR are not prerequisites. The full canonical-corpus study still needs M4.
3. M4 serialization can start with the existing types. Future type/effect changes
   require versioning; no need to implement all M3 types first. Add a portable MIR
   only for an independent consumer or a concrete validation/optimization requirement.
4. LLVM has been selected and measured. NIL-100's requirement to implement three
   backend prototypes is superseded as an immediate prerequisite by ADR 012.
   Revisit Cranelift/WASM only for measured latency, deployment or portability needs.
5. Compare syntax under identical arithmetic and budget policy. V3 shares v2's
   grammar but changes arithmetic/execution policy; v2/v3 timing differences are
   not evidence about syntax. More syntax forms must have an experimental hypothesis.
6. A minimal M8 plugin can use current primitive types and validated HIR after its
   identity, typing, effects and lowering contract is specified. It need not wait
   for every aggregate type or a public MIR serialization.

## Recommended next work

Prepare a small M6 pilot using the existing integer/control-flow core: freeze
held-out paired tasks and execution policy, add versioned per-attempt logs,
mock-test failures/repairs/usage accounting, then run budgeted model trials after
model access and cost are specified. Report Solve@budget and correctness alongside
failure-inclusive TCR. This is a recommendation, not authorization for paid runs.

In parallel or afterward, execute NIL-030 to select the smallest type extension
needed by new acceptance programs. No type, syntax or backend expansion is required
merely to complete the numbered roadmap. M8/M9 test semantic compression later;
M11/M12 remain gated investigations. Negative benchmark findings are valid results.
