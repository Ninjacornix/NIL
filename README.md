# NIL — Neural Instruction Language

NIL is a research language / semantic IR for LLM-generated programs. The objective
is fewer total model tokens to a correct program, with strong static checks and a
small deterministic compiler. Shorter source alone is not evidence of improvement.

**Milestone 1 is implemented:** source → AST → signature/type checking → validated
HIR → reference interpreter. Supports i64 constants, typed functions/parameters,
add/sub/mul/div, calls and returns. No control flow, memory, plugins or native codegen.
The `lines-v0` syntax is an experiment, not a selected token-optimal representation.

## Build, run and inspect

Requires Rust 1.85 or newer. No third-party Rust dependencies or network access needed.

```sh
cargo build --workspace --locked --offline
cargo run -p nil --offline -- run examples/add.nil
# prints 42
cargo run -p nil --offline -- run examples/add.nil 1 20 22
# calls function 1 with parameters; prints 42
cargo run -p nil --offline -- check examples/add.nil
cargo run -p nil --offline -- hir examples/add.nil
```

The [example](examples/add.nil) calls an addition function from entry function 0.
Each instruction produces the next local value ID; parameters receive IDs first.
See the [grammar and semantics](docs/language/V0_1.md). Arithmetic traps on overflow
and invalid division. Evaluation is bounded to 100,000 steps and 256 call frames;
library callers can configure limits. Programs are limited to 1 MiB of source.

## Test and benchmark

```sh
cargo test --workspace --locked --offline
cargo test --workspace --release --locked --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo bench -p nil-compiler --bench pipeline --locked --offline
```

31 tests cover parsing, malformed input, signatures, value scope, independent HIR
validation, lowering, diagnostics, arithmetic traps, bounded recursion and CLI
execution. Debug/release runs must agree. CI checks stable and Rust 1.85.
The benchmark reports frontend and interpreter latency separately as JSON;
unavailable tokenizer/native metrics remain null. It makes no LLM-efficiency claim.

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
M2 control flow and all later milestones remain planned; this pass stops at M1.
