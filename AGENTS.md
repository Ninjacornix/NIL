# Repository Guidelines

## Project Structure & Module Organization

NIL is a Rust compiler workspace. `crates/nil-compiler/` owns parsing, checking,
HIR lowering and reference execution; `crates/nil-hir/` defines typed operations
and validates semantic invariants. `crates/nil-llvm/` emits LLVM IR and includes
the native C application runtime. The CLI lives in `cli/nil/`; seeded fuzzing
lives in `tools/nil-fuzz/` and `fuzz/corpus/`. Specifications and architectural
decisions live under `docs/`. Keep surface syntax separate from semantic HIR.

## Build, Test, and Development Commands

Install the pinned Rust toolchain via `rustup show active-toolchain` and Clang
15+ (plus LLD for Linux v5 O2); use `NIL_CLANG` to select Clang. Run commands from the repository root:

- `./scripts/ci.sh`: formatting, strict Clippy, build, integration tests and doctests.
- `./scripts/ci.sh release`: build and test optimized Rust binaries.
- `cargo fmt --all`: apply standard Rust formatting.
- `cargo run -p nil -- run examples/add.nil`: execute the default source profile.
- `./scripts/sanitize.sh`: run native application tests with ASan and UBSan.

Cargo commands use `--locked --offline`; commit lockfile changes deliberately.
Benchmark suites are an external `benchmarks/` submodule; initialize it with
`git submodule update --init benchmarks` before using `scripts/bench.sh`.

## Coding Style & Naming Conventions

Use rustfmt, four-space Rust indentation, snake_case modules/functions and
PascalCase types. Prefer small modules, explicit data structures, deterministic
behavior and few dependencies. Preserve earlier source profiles and the default
profile. Syntax experiments require explicit profile selection and documentation.

## Testing Guidelines

Use Rust's built-in test runner; integration tests live in each crate's `tests/`.
Name tests after verified behavior. Cover malformed input, diagnostic codes/spans,
HIR validation, evaluator semantics and native execution at O0/O2. Every fixed
compiler bug needs a regression test. Review changed golden fixtures. Native
tests require Clang and must not silently skip. Differential fuzzing must preserve
failed seeds and compare host effects as well as returned values.

## Commit & Pull Request Guidelines

Use Conventional Commits: `<type>(<scope>): <description>`. Explain semantic
changes and link the relevant specification or ADR. Never invent issue IDs.
PRs need purpose, changes and exact validation results, including checks that
could not run. Merge PRs using Squash and merge. Keep credentials, generated
binaries, local environments and benchmark results out of version control.
