# Milestone 3 delivery — expr-v4

The user-requested upgrade of expr-v3 is implemented on `feat/expr-v4-types`.
This is a selected semantic experiment, not a claim of universally optimal syntax
or general-purpose completeness. Earlier profiles and the expr-v0 default remain.

## Requirements and evidence

| Requirement | Delivered evidence |
|---|---|
| Select a small test-driven type extension | [Type requirements](milestones/03-types.md), [ADR 014](adr/014.md), [v4 contract](language/EXPR_V4.md): bool signatures and immutable fixed i64 arrays; widths/floats/references explicitly deferred |
| Keep syntax separate from semantics | Lexer/parser produce AST; `nil-hir` adds syntax-independent types/operations/validation; evaluator and LLVM consume validated HIR |
| Implement new capabilities end-to-end | Constructors/repeat/length/read/replacement, exact typed calls/returns, array/bool branches and loop state, native flat entry bridge and typed output |
| Preserve deterministic safety semantics | Wrapping v3 arithmetic; E012 checked bounds; no implicit conversion; lazy branches; simultaneous updates and immutable aliases; snapshot storage never reused for writes |
| Verify positive/negative behavior | Compiler v4 tests, HIR validation cases, reviewed array HIR/diagnostic goldens, native O0/O2 comparisons in both budget modes, typed CLI tests |
| Retain older source profiles | Existing suites plus identical v3/v4 HIR for every v3 example; v4 syntax rejected by older profiles |
| Property/mutation testing | Independent seeded sum/reverse arrays of length 0..256; 6000 typed source mutations and 1000 typed HIR mutations; existing v3 campaigns remain reproducible |
| Measure actual token efficiency | Two pinned tokenizers, characters/bytes/hashes for 25 equivalent programs, signature alternatives and v3 unrolled sum screens; model repair/TCR explicitly unmeasured |
| Compare speed fairly | Native O0/O2, C++ O2 and Python oracle fixtures; identical native C driver and flat ABI; fresh-local mutation baselines reveal copying costs |
| Measure compilation and payload separately | Frontend/IR/codegen/runtime/link/size fields; independent repeated HIR checker timings and flat input bytes |
| Explain discoveries and update plans | [Measured report](../benchmarks/paired/results/2026-09-30/EXPR_V4.md), project state, roadmap, architecture, ADR and M3/M10 follow-ups |

## Validation performed

All commands completed successfully locally on macOS/Apple Silicon:

```sh
./scripts/ci.sh                         # format, Clippy, all-target build; 148 Rust tests
./scripts/ci.sh release                 # release build; same 148 tests
cargo +1.85.0 check --workspace --all-targets --locked --offline
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/paired/tests
# 19 benchmark tests
```

All eleven `examples/expr-v4` programs were executed through the release CLI with
explicit inputs and checked outputs. Benchmark commands/settings are preserved in
[the report](../benchmarks/paired/results/2026-09-30/EXPR_V4.md#reproduce); the 79
primary fixtures pass all four implementations (316 comparisons), and eight
fresh-local transform variants pass their independent fixture checks. Current
recorded source hashes were checked against the working tree.

## Findings and remaining scope

NIL uses 851/906 tokens versus Python's 1068/1068 with fresh-local transforms
(about 20%/15% fewer) in this corpus. Numeric type lengths win the tested signature
screen, but length-8 unrolled sums are shorter than array loops. No model-generation
success, repair cost or TCR was measured.

Read-only parameter snapshots fix repeated copying without changing semantics.
The initial 256-element reverse/prefix measurements exposed quadratic copying
(roughly 147×/105× slower than fresh-local C++). [ADR 015](adr/015.md) now implements
private loop storage for proven single-use replacement chains, including guarded
updates, and internal typed-function inlining. Independent native regressions cover
aliases, old reads, lazy bounds traps, simultaneous state updates and exact budgets.
The storage report remeasures 25 original and nine additional kernels against
fresh-local baselines with a per-kernel 1.25× C++ speed gate. Historical reports
remain intact. No source spelling or observable value semantics changed. Linux
execution relies on existing CI and has not been run locally in this pass.

Final evidence: [storage benchmark](../benchmarks/paired/results/2026-09-30/EXPR_V4_STORAGE.md) — all 34 kernels pass the 1.25× C++ gate;
combined source uses 1115/1194 NIL versus 1425/1425 Python tokens. Generation/TCR
remains unmeasured.
