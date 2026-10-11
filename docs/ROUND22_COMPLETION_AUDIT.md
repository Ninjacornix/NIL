# Round 22 completion audit

The delivered result is **guarded G2 with fallback, no G1**, under the round's
explicit soundness stop condition. Natural qualification remains **19/150**,
tuned **22/150**; the forecasts largely missed. This audit does not claim that
G1's eight obligations were discharged or that its implementation exists.

The maintainer subsequently accepted nullable keeping-read diagnostics with
separate retention evidence and a reason. Full semantic-observation tracing is
not required by the clarified Part D contract. See [ADR 044](adr/044.md).

## Requirement-to-evidence mapping

| Requirement | Result and authoritative evidence |
|---|---|
| Branches from the required bases, local commits only | `feat/core-ownership` descends from `92d62b3`; `exp/core-ownership` from `84a20fc`; both have no upstream. No push was executed. |
| Part A: execute fuzz census rather than stop at E006 | Benchmark `76af011`, main `eab1320`, precede ADR/code. Four typed arguments, Host fixtures and providers supplied by the harness. Both seeds execute all 644 original programs; no skipped/unjoinable cases, one bounded E008 each. |
| BEFORE baseline committed first | `eab1320` and benchmark BEFORE JSON are earlier than `46c10fc` and `d97b481`. Later version-birth instrumentation repair is disclosed; corrected retrospective before/after and original forecasts both retained. |
| ADR and forecasts before compiler code | ADR 044 `46c10fc` precedes G2 `d97b481`, initial hints `bce3d6e`. Diagnostic follow-up `2b7a127` precedes `9fee03b`; nullable-contract amendment `99b4e9d` precedes its renderer. |
| General G2 and all eight obligations | Typed SSA interval analysis plus guarded scalar replay, not a source-shape recognizer. The report's obligation matrix explains retained aliases/roots, scalar-only caching, selected lazy arms, recursion/work-budget fallback, child reachability, trap/effect barriers, unchanged quota and deferred roots. Unsupported paths execute the original copying program. |
| G1 after G2 in a separate commit | **Not admitted**, as explicitly allowed by the stop condition. Parameter/returned/child provenance and logical charge/death proof are absent. No narrower callee matcher was substituted. |
| Concrete conflict for G1 / wider G2 | Two quota witnesses require E013 at `39..47` and `100..101`. One needs three semantic versions; the other retains an unread byte child through a later scalar parent-field read. Releasing roots based solely on payload last use changes E013. This refutes the proposed early-release shortcut, not every possible ownership design. |
| Part D: deterministic actual-copy diagnostics | Runtime emits successful C copy paths only, first occurrence per kind/site. Local keeping reads, cross-frame/child retention uses and explicit reason tags are distinct. Missing semantic provenance stays null under the maintainer-accepted contract. O0/O2 × bounded/unbounded tests compare outputs, effects, codes/spans and repeated hints. |
| Part D measurement boundary | Nominal C-path bytes are not O2 machine traffic. Construction/input, allocator relocation and SSA aggregate moves are outside this diagnostic. The separate census supplies totals; root metadata does not admit ownership. |
| Full variant-matrix soundness tests | Fourteen native read-replay/diagnostic tests and 29 named `g2_` fuzz families cover operand order, different indices, bindings, lazy arms, reads before/after writes, aliases/returns/calls, recursion/mutual recursion, record/map children, quota/effects/traps, empty/zero-slot values and exhausted replay/provenance budgets. Unsupported patterns test fallback, not a claimed ownership gain. |
| AFTER census in all required scopes | `ROUND22_CENSUS.json`: application 68, admitted natural/tuned, fuzz seeds 22001/22002; events/bytes and REQUIRED/AVOIDABLE cuts, per-program/worst results, exclusions and hashes. Corrected reference ledgers agree before/after. No O2 copy census claimed. |
| Natural/tuned qualification and five cliffs | `ROUND22_FOLLOWUP_COHORT.json`: 29 admitted programs each; 15 rotating timing repetitions and separate three-run RSS, on unchanged largest fixtures with same-algorithm C++. Sets remain exactly 19 and 22; no new qualification. |
| Historical controls and transform drift | Initial paired control plus two retained follow-up sets, 25 repetitions each. Append/scan stable; old/current/C++ transform timings all move. Exact absolute-drift cause remains unknown. No claim of a compiler regression, zero overhead, thermal cause or faster code from the altered C++ ratio. |
| CI, sanitizers and differential campaigns | 526 debug/release tests, 237 sanitizer tests, zero warnings; output hashes in the contract receipts; campaigns 22031/22032/22033: 673 indices each, 2,019 programs / 4,038 O0/O2 builds, zero divergences. Per-code/family counts retained; host effects and diagnostic spans compared. E013 38/seed; unbounded campaigns do not hit E008, bounded census and limit tests do. |
| Frozen corpus and cohorts | Application 2,815 checks; original separately exactly 610. Natural/tuned each 1,070/1,070. Source hashes match; benchmark diff changes no `corpora/` or `natural/` input. Seven blocked sampled candidates and 114 held-out problems untouched. |
| Limits / stack safety / other scope exclusions | No limits, default-profile, grammar, intrinsic, dependency or earlier-profile change. Round 23 memory/depth/stack work remains deferred. |

## Evidence and residual risks

The [full report](../benchmarks/reports/2026-10-08/ROUND22.md) preserves initial
and follow-up measurements, forecast misses, all code/family counts and the raw
archive's SHA-256 manifest. Timings/RSS ran alone, without CI/fuzz/census jobs.
The instrumentation's cross-kind address-reuse defect was fixed with regressions;
that was a measurement bug, not a compiler divergence. No unexplained divergence
or sanitizer finding is being shipped.

The final diagnostic contract adds a reason tag only under
`NIL_EXPLAIN_COPIES`. Clang-preprocessed default C runtime tokens match before and
after with record buffers both enabled and disabled; Rust codegen is unchanged.
Previously completed default-mode corpus, fuzz and performance evidence applies
to this exact unchanged compilation path. CI and sanitizers are rerun for the
changed opt-in renderer and strengthened quota assertions. Do not relabel earlier
campaigns as fresh runs of the amended diagnostic build.

Root retention does not prove payload observation. Native observed-root charge
is not semantic quota headroom. Fuzz families are mostly fixed adversaries, not
broad random ownership synthesis. Diagnostic traversal/expansion budgets remain
explicit and exhausted provenance stays null. G1 still needs a new complete
provenance and quota-equivalence argument before implementation; the present
results are not evidence that such an analysis is impossible.

Final contract receipts: [ROUND22_CONTRACT_GATES.json](../benchmarks/reports/2026-10-08/ROUND22_CONTRACT_GATES.json).
Earlier full default-mode campaign receipts: [ROUND22_FOLLOWUP_GATES.json](../benchmarks/reports/2026-10-08/ROUND22_FOLLOWUP_GATES.json).
