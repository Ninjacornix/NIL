# NIL — Neural Instruction Language

NIL is a research language / semantic IR for LLM-generated programs. The objective
is fewer total model tokens to a correct program, with strong static checks and a
small deterministic compiler. Shorter source alone is not evidence of improvement.

**Milestone 1 is implemented:** source → AST → signature/type checking → validated
HIR → reference interpreter. Supports i64 constants, typed functions/parameters,
add/sub/mul/div, calls and returns. No control flow, memory, plugins or native codegen.
The `lines-v0` syntax is an experiment, not a selected token-optimal representation.
An optional [expression profile](docs/language/EXPR_V0.md) is available for
paired syntax experiments.

## Build, run and inspect

Development uses pinned Rust 1.98.1 via rustup; MSRV is 1.85.0. No third-party Rust
dependencies or network access are needed after toolchain installation.

```sh
cargo build --workspace --locked --offline
cargo run -p nil --offline -- run examples/add.nil
# prints 42
cargo run -p nil --offline -- run examples/add.nil 1 20 22
# calls function 1 with parameters; prints 42
cargo run -p nil --offline -- check examples/add.nil
cargo run -p nil --offline -- hir examples/add.nil
cargo run -p nil --offline -- --profile expr-v0 run benchmarks/paired/samples/affine.expr.nil 0 20 22
```

The [example](examples/add.nil) calls an addition function from entry function 0.
Each instruction produces the next local value ID; parameters receive IDs first.
See the [grammar and semantics](docs/language/V0_1.md). Arithmetic traps on overflow
and invalid division. Evaluation is bounded to 100,000 steps and 256 call frames;
library callers can configure limits. Programs are limited to 1 MiB of source.

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
runs nightly and reports JSON; unavailable token/backend metrics remain null.
The [paired benchmark](benchmarks/paired/README.md) compares exact source tokens
under two pinned tokenizers and already-compiled NIL interpreter calls with
equivalent Python functions.
Run it with `--manifest benchmarks/paired/cases-expr.json` to measure `expr-v0`.
It is a local experiment and is not part of CI.

## Repository map

- `crates/nil-hir/`: semantic types/operations, diagnostics, validator and tests.
- `crates/nil-compiler/`: syntax AST, parser, lowering, evaluator, tests and benchmark.
- `cli/nil/`: check/run/hir CLI and integration tests.
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
M2 control flow and the full later milestones remain planned. `expr-v0` is an
early syntax experiment, not completion of the M7 generation study.
