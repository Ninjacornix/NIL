# Contributing to NIL

Keep the default branch buildable and tested. Work on a topic branch and open a PR
for significant compiler changes. Do not change language semantics to satisfy CI.

## Setup and local checks

Install Git, Clang 15+ with system C headers/linker, and Rust through rustup.
Ubuntu CI installs clang-18; set NIL_CLANG to choose the Clang executable. Native
codegen tests require Clang and never silently skip/fall back to interpretation. `rust-toolchain.toml` selects Rust 1.98.1 with
rustfmt and Clippy, matching CI. `rustup show active-toolchain` installs that pinned
toolchain when needed. Rust 1.85.0 remains the minimum supported version (MSRV),
checked nightly. Update the development pin deliberately in a tested PR; do not
use floating stable/nightly for mandatory checks. Cargo.lock is committed and the
workspace currently has no external Rust dependencies. After toolchain installation,
all Cargo checks run locked and offline.

From the repository root, run the same mandatory checks as PR CI:

```sh
./scripts/ci.sh
```

Requires Bash (available on Linux/macOS). Individual commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo build --workspace --all-targets --all-features --locked --offline
cargo test --workspace --lib --bins --tests --all-features --locked --offline
cargo test --workspace --doc --all-features --locked --offline
```

Fix formatting with `cargo fmt --all`. Fix warnings rather than suppressing them
globally. All features are checked; the workspace currently declares none. Tests
explicitly select libraries, binaries, integration tests and doctests instead of
`--all-targets`: the latter also executes the custom `harness = false` performance
benchmark. Build and Clippy still cover benchmark targets on every PR.

Focused compiler checks:

```sh
cargo test -p nil-compiler --test programs --locked --offline
cargo test -p nil-compiler --test compile_fail --locked --offline
cargo test -p nil-compiler --test pipeline --locked --offline
cargo test -p nil-hir --test validation --locked --offline
cargo test -p nil --test cli --locked --offline
./scripts/ci.sh release
```

`programs` compiles fixture sources through parsing, checking and validated HIR to
execution. `compile_fail` checks diagnostic codes, phases, source spans, and
expected/actual values where available. Fixtures live under
`crates/nil-compiler/tests/fixtures/{programs,fail}`. Add tests only for supported
features: earlier profiles have i64 function signatures; expr-v4 adds bool
functions and immutable fixed-length i64 arrays. Keep unsupported widths, floats
and exposed memory rejected. `expr_v4` covers typed calls/returns, aliasing, bounds,
aggregate invariants and seeded oracle comparisons; native tests exercise O0/O2
and both budget modes.

`pipeline` covers arithmetic traps, bounded execution, malformed input and the
existing deterministic HIR/diagnostic goldens. HIR dumping is a debugging projection,
not a stable serialization ABI; review golden diffs deliberately. Do not snapshot
spans or Rust debug internals as a future canonical IR format. `validation` checks
externally constructed HIR: missing values/returns, forward/self references,
invalid functions, call arity/arguments and validation of unused functions. The
compiler already validates HIR before execution; only `ValidatedProgram` can enter
the evaluator. Preserve this boundary.

CLI tests execute every `examples/*.nil` with entry function 0 and no arguments,
compare against the accompanying `.stdout`, and require an empty stderr. Add the
expected result with every new example; the suite fails for missing expectations.
These tests run in both debug and release, along with the full compiler suite.

## CI levels and branch protection

CI currently targets Linux (`ubuntu-24.04`). Local validation also runs on macOS;
macOS and Windows are not yet continuously tested or promised release targets.
No deployment, publishing or release uploads happen automatically.

- **PRs targeting main:** mandatory `quality` job: format, Clippy, all-target build,
  unit/integration/compile-fail tests, HIR invariants, goldens and doctests. One job
  avoids repeated setup for this small dependency-free workspace. Individual steps
  identify the failure; there are no placeholder MIR/token jobs.
- **Pushes to main:** the same job followed by `release`, which builds all release
  targets and runs the full release suite, including all examples.
- **Nightly (03:23 UTC) and manual dispatch:** repeat mandatory and release checks,
  report the existing compiler benchmark, and build/test on MSRV 1.85.0. Scheduled
  workflows run on GitHub's default branch and become active after merging there.
  CI also supports manual dispatch for checking a topic branch.

The repository currently defaults to `master`. Both workflows support the current
setup; PR/push CI includes `master` during migration. Rename the default branch to
`main` in repository settings when ready, then remove the transitional `master`
filters. This task does not rename the remote branch.

Owner configuration in GitHub Settings → Rules → Rulesets (or Branches → protection):

1. Target `main`; apply the same protection to `master` until migration.
2. Require a pull request before merging. For a solo owner, zero required approvals
   permits self-authored PRs; enable one approval when another reviewer is available.
3. Require status check **`quality`** from GitHub Actions and require branches to be
   up to date before merging. This single required check covers formatting, Clippy,
   build, unit/integration, compile-fail and HIR validation. Select the check after
   its first PR run; do not require push-only `release` or nightly jobs on PRs.
4. Require resolved review conversations, block force pushes and branch deletion,
   and disallow bypass/direct pushes (including admins, if appropriate).
5. In Settings → General → Pull Requests, enable **Allow squash merging** and
   disable **Allow merge commits** and **Allow rebase merging**. Use a Conventional
   Commit-style PR title; GitHub uses it for the squash commit title. Review and
   edit the final squash message before merging. This repository setting, rather
   than a workflow check, makes squash the only available PR merge method.
6. Keep merge queue disabled until `merge_group` is explicitly wired into CI.

Protection is configured on GitHub, not by these files. PR runs cancel older runs
for that same PR; distinct push runs never cancel each other. Workflow permissions
are read-only; checkout does not persist credentials. No secrets or privileged
`pull_request_target` runs are used. Actions are pinned to full commit SHAs with
release annotations. Review upstream changes when updating pins. Only build outputs
are cached; cache keys include OS, architecture, toolchain, manifests and lockfile.
Mandatory checks have no failure suppression and jobs have bounded timeouts.

## Benchmarks and future checks

```sh
cargo bench -p nil-compiler --bench pipeline --locked --offline
```

The existing benchmark warms up then measures 10,000 frontend and interpreter runs
on `examples/add.nil`. It emits JSON with unavailable token/backend metrics as null.
Nightly preserves JSONL and toolchain/OS/CPU/commit provenance for 14 days. Benchmark
failure is explicitly non-blocking and reported in the workflow summary; correctness
checks remain blocking. No noisy timing threshold or efficiency claim is imposed.
See [measurement protocol](benchmarks/README.md).

TODOs when their owning milestones land (no always-success placeholder commands):

- **MIR (M4):** validate after lowering, before execution/codegen; test definitions,
  block targets/terminators, dominance, branch arguments, types and call signatures.
  Add canonical round-trip/idempotence goldens once serialization is versioned.
- **Tokenbench (M5):** small informational PR comparison, extended nightly corpus;
  retain raw JSONL with pinned tokenizer revisions and baseline/current/change,
  average and worst regression. No merge threshold until one is accepted.
- **Performance:** split parse/check/HIR construction/validation timings when APIs
  support it; add MIR lowering/validation and native codegen timing when implemented.
- **Fuzzing:** expr-v3 mutation/property and native differential campaigns run nightly;
  see [FUZZING.md](docs/FUZZING.md). Coverage-guided engines, memory isolation and
  HIR/MIR deserialization/plugin targets remain follow-ups as those interfaces appear.
- **Larger corpora/backends/platforms:** extend nightly first when implementations
  and support commitments exist; keep PR checks within a few minutes.
- **LLM generation (M6):** separate opt-in evaluation, never mandatory paid API calls.
  Report syntax/type/compile/semantic success, generated/repair tokens, repair rounds
  and total tokens to correct program under the frozen measurement protocol.

## Branches, commits and PRs

```sh
git switch -c chore/describe-change
git config --local commit.template .gitmessage
```

Use `<type>(<scope>): <description>` in imperative form. Types: `feat`, `fix`,
`refactor`, `perf`, `test`, `docs`, `bench`, `exp`, `chore`. Scopes include `parser`,
`syntax`, `types`, `hir`, `mir`, `compiler`, `runtime`, `plugin`, `codegen`, `cli`,
`tokenbench`, `tokens`, `spec`, and `ci` for infrastructure. Describe what changed,
not what the agent did. Add an optional explanation and `Refs: NIL-<issue>` only
when an actual issue applies. Examples:

```text
feat(parser): add function declaration parsing
test(hir): reject invalid value references
chore(ci): add nightly compiler validation
```

Before pushing, run `./scripts/ci.sh`. Merge approved PRs with **Squash and merge**
so each PR contributes one descriptive commit to the default branch. PRs explain
the problem, behavior, relevant
issue IDs, validation performed, and changed assumptions. Include fixture/golden
diffs with compiler changes and regression tests for fixes. Keep credentials and
local environment files out of commits. Separate language-design changes from CI.

## Native backend checks

`cargo test -p nil-llvm --locked --offline` runs LLVM O0/O2 differential tests.
The default CLI now compiles native code. Reference HIR execution is retained
through library tests and benchmark tools. See [native commands and ABI](docs/language/NATIVE_LLVM.md).

For expr-v3 changes, test both wrapping execution and optional bounded instrumentation;
verify signed boundaries at O0/O2 and keep earlier profiles' checked diagnostics intact.
The C++ comparison command is documented in [benchmarks/paired](benchmarks/paired/README.md).
Report source tokens separately from runtime and never infer TCR from either.

## Fuzz testing

Run `./scripts/fuzz.sh` after expr-v3 compiler changes. Use a dedicated `--out` directory
and preserve the seed/source/mode from failures. See [FUZZING.md](docs/FUZZING.md)
for long campaigns and replay. Promote each fixed bug into a regression test/corpus.
The normal workspace suite already runs deterministic property and native smoke tests.


## Benchmark checks

```sh
./scripts/bench.sh token --smoke
./scripts/bench.sh runtime --smoke
./scripts/bench.sh generation --mock
python3 -m unittest discover -s benchmarks/tests -v
```

These checks require no model or historical results. Runtime smoke needs Clang.
Use [the benchmark guide](benchmarks/README.md) for locked tokenizer setup, optional
full measurements and output directories. Share raw runs as artifacts; commit only
reviewed fixtures, tooling and concise published findings.
