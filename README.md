# NIL — Neural Instruction Language

NIL is a research language / semantic IR for LLM-generated programs. The objective
is fewer total model tokens to a correct program, with strong static checks and a
small deterministic compiler. Shorter source alone is not evidence of improvement.

**Status: repository setup and planning only.** The Rust CLI displays help and
version information. No NIL parser, type checker, interpreter, native backend or
plugin runtime is implemented. The current pass intentionally stops before M1.

## Build and check

Requires Rust 1.85 or newer with Cargo, rustfmt and Clippy. No third-party Rust
crates, network access or model credentials are needed for local build/test.

```sh
cargo build --workspace --locked --offline
cargo test --workspace --locked --offline
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo run -p nil --offline -- --help
cargo run -p nil --offline -- --version
```

CI checks the declared minimum and stable Rust, plus formatting and linting.
Use standard rustfmt, four-space Rust indentation, `snake_case` modules/functions
and `PascalCase` types. Keep compiler changes small and add regression tests for fixes.
Use Conventional Commits (`feat(parser): add function declarations`) following
[the commit guidelines](AGENTS.md#commit--pull-request-guidelines). Enable the
repository's default message template once per clone:

```sh
git config --local commit.template .gitmessage
```

Run `git commit` to open the template in your editor. Its commented guidance is
removed from the final message; supply a specific subject, an optional explanation,
and `Refs: NIL-<issue>` when applicable.

PRs should include intent, relevant task IDs, tests and changed assumptions.

## Repository map

- `cli/nil/`: buildable CLI scaffold and integration smoke tests.
- `docs/about/misc/`: original research, preserved verbatim.
- `docs/PROJECT_STATE.md`: research synthesis, conflicts and open decisions.
- `docs/ROADMAP.md`, `docs/milestones/`: implementation order and 13 detailed plans.
- `docs/architecture/`, `docs/language/`: proposed compiler boundaries and M1 core.
- `docs/adr/`: accepted constraints and provisional decisions.
- `benchmarks/README.md`: TCR/TTCP definitions and measurement protocol.
- `.github/workflows/ci.yml`: build/test/lint basics.

M1 will create `crates/nil-hir/` and `crates/nil-compiler/` when they contain working
semantics and frontend/evaluator code. Later milestones introduce MIR, tokenbench,
plugins and backends only when their boundaries are needed. No empty placeholder
crates or fabricated benchmark results are included.

## Start developing

Read [project state](docs/PROJECT_STATE.md), then the [roadmap](docs/ROADMAP.md).
Start with NIL-010 in the [M1 plan](docs/milestones/01-minimal-executable.md).
Its acceptance target is source → AST → type checking → validated HIR → interpreter,
computing `add(20, 22) = 42`. The [provisional grammar](docs/language/V0_1.md) is an
experiment, not a selected token-optimal syntax. Research conflicts stay explicit.

Keep TCR and correctness as the primary experimental evidence; tokenizers, model
adaptation, semantic frameworks and self-hosting are later investigations.
