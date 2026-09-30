# NIL — Neural Instruction Language

NIL is a research language / semantic IR for LLM-generated programs. The objective
is fewer total model tokens to a correct program, with strong static checks and a
small deterministic compiler. Shorter source alone is not evidence of improvement.

**Milestones 1 and 2 are implemented:** source → AST → signature/type checking → validated
HIR → LLVM → native executable (default). Supports i64 constants, typed functions/parameters,
add/sub/mul/div, calls, returns, bool comparisons, lazy branches and typed loops.
See [control-flow semantics](docs/language/CONTROL_FLOW.md). Memory and plugins remain future work. See [native build requirements](docs/language/NATIVE_LLVM.md).
The default [expression profile](docs/language/EXPR_V0.md), `expr-v0`, uses arithmetic expressions and function calls. `lines-v0` remains available with `--profile lines-v0` for compatibility and paired comparisons. A small local comparison found fewer source tokens for `expr-v0` on three arithmetic examples. Model generation and repair still need evaluation before choosing a final syntax.

## Download the compiler

macOS releases provide standalone executables for Apple Silicon and Intel.
See [release installation and publishing](docs/RELEASES.md); downloaded binaries
do not require Rust.

## Build, run and inspect

Development uses pinned Rust 1.98.1 via rustup; MSRV is 1.85.0. Native builds/runs
and the full test suite require Clang 15+, system C headers and a linker. Rust
dependencies are workspace-only; no network access is needed after tool installation.

```sh
cargo build --workspace --locked --offline
cargo run -p nil --offline -- run examples/add.nil
# prints 42
cargo run -p nil --offline -- run examples/add.nil 1 20 22
# calls function 1 with parameters; prints 42
cargo run -p nil --offline -- build examples/add.nil -o /tmp/nil-add
/tmp/nil-add
# standalone native program; prints 42
cargo run -p nil --offline -- llvm examples/add.nil
cargo run -p nil --offline -- check examples/add.nil
cargo run -p nil --offline -- hir examples/add.nil
cargo run -p nil --offline -- run benchmarks/paired/samples/affine.expr.nil 0 20 22
```

The [example](examples/add.nil) calls an addition function from entry function 0.
The frontend assigns local value IDs; parameters receive IDs first.
See the [expression grammar](docs/language/EXPR_V0.md) and
[legacy line syntax](docs/language/V0_1.md). Earlier profiles trap on overflow and invalid division; v3 wraps overflow
and retains a division-by-zero trap. Evaluation is bounded to 100,000 steps and 256 call frames;
library callers can configure limits. Programs are limited to 1 MiB of source.

Experimental compact profiles `expr-v1`, `expr-v2` and `expr-v3` are available explicitly
with `--profile`. [expr-v3](docs/language/EXPR_V3.md) uses wrapping integers and
omits resource counters by default for C++-class native execution; select
`--bounded` for validation/repair runs. Earlier profiles keep checked arithmetic.
See [grammar and tradeoffs](docs/language/EXPR_COMPACT.md) and
[measured token/runtime comparisons](benchmarks/paired/results/2026-09-30/README.md).

## Test and benchmark

```sh
./scripts/ci.sh          # same mandatory checks as PR CI
./scripts/ci.sh release  # full suite and examples in release mode
cargo fmt --all          # fix formatting
cargo bench -p nil-compiler --bench pipeline --locked --offline
uv run --project benchmarks/paired --locked python benchmarks/paired/run.py
```

Tests cover parsing, malformed input, structured compile-fail diagnostics, program
fixtures, signatures, HIR invariants, golden output, arithmetic traps, bounded
recursion and CLI execution. Debug/release runs must agree. See
[CONTRIBUTING.md](CONTRIBUTING.md) for focused commands, toolchain policy, CI levels,
branch protection, and future tokenbench/fuzzing integration. Compiler performance
runs nightly and reports JSON; unmeasured fields remain null in that frontend report.
The [paired benchmark](benchmarks/paired/README.md) compares exact source tokens
under two pinned tokenizers and reference-interpreter calls with equivalent Python
functions. [LLVM native measurements](benchmarks/paired/results/2026-09-30/NATIVE_LLVM.md)
compare O0/O2 code, the reference interpreter and Python separately.
Run `run.py --manifest benchmarks/paired/cases-expr.json` to measure `expr-v0`,
or `native.py --output /tmp/native.json` for the native comparison.
Performance measurements are local experiments, separate from mandatory native CI tests.

## Repository map

- `crates/nil-hir/`: semantic types/operations, diagnostics, validator and tests.
- `crates/nil-compiler/`: syntax AST, parser, lowering, evaluator, tests and benchmark.
- `crates/nil-llvm/`: LLVM SSA emission, host native builds and differential tests.
- `cli/nil/`: check/hir/llvm/build/run CLI and integration tests.
- `examples/`: executable NIL programs.
- `docs/about/misc/`: original research, preserved verbatim.
- `docs/PROJECT_STATE.md`: decisions, conflicts and open questions.
- `docs/ROADMAP.md`, `docs/milestones/`: 13 detailed milestone plans.
- `docs/architecture/`, `docs/language/`, `docs/adr/`: architecture, spec and decisions.
- `benchmarks/README.md`: TCR/TTCP methodology.
- `benchmarks/paired/`: executable NIL/Python source and runtime comparison.

## Contributing and next steps

Use rustfmt, four-space Rust indentation, `snake_case` functions/modules and
`PascalCase` types. Keep changes focused; add regression tests for compiler fixes.
Use Conventional Commits (`feat(parser): add function declarations`) following
[the commit guidelines](AGENTS.md#commit--pull-request-guidelines). Enable the
repository's default message template once per clone:

```sh
git config --local commit.template .gitmessage
```

Run `git commit` to open the template in your editor. Its commented guidance is
removed from the final message; supply a specific subject, an optional explanation,
and `Refs: NIL-<issue>` when applicable.

PRs should cite task IDs, explain intent,
and report tests and changed assumptions. No coverage percentage substitutes for
semantic invariants and invalid-program tests.

Read [project state](docs/PROJECT_STATE.md) and the [roadmap](docs/ROADMAP.md).
[NIL-010–014 are complete](docs/milestones/01-minimal-executable.md).
[M2 control flow is complete](docs/milestones/02-control-flow.md); later full milestones remain planned. M7 is partially complete; model-generation and repair measurements remain.

LLVM is now the default execution backend; the reference evaluator remains an oracle.
The full M4/M10 milestones remain partial beyond this bounded native path.
