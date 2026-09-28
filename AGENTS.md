# Repository Guidelines

## Project Structure & Module Organization

This directory is currently an empty project workspace. No source code, tests, assets, dependency manifests, or build configuration exist yet. Introduce a structure that fits the chosen language and framework, and document it in `README.md`.

For a simple initial layout, use `src/` for application code, `tests/` for automated tests, and `assets/` for static resources when needed. Keep related modules together and avoid creating unused directories.

## Build, Test, and Development Commands

No build, test, or development commands are configured. When adding the initial toolchain, provide reproducible commands in `README.md` for dependency installation, local development, testing, and production builds where applicable.

Prefer project scripts or task targets over undocumented shell sequences. Commit the appropriate dependency lockfile and document required runtime versions.

## Coding Style & Naming Conventions

No language-specific style or formatter has been selected. Follow the chosen ecosystem’s standard conventions and configure its formatter and linter when introducing source code. Use consistent indentation within each file, descriptive identifiers, and filenames that reflect module responsibilities. Keep changes focused and avoid unrelated formatting edits.

## Testing Guidelines

No testing framework or coverage threshold exists. Add an appropriate test runner with the first testable functionality. Name tests after the behavior they verify and follow the runner’s discovery conventions. Cover new behavior and add regression tests for bug fixes. Document the exact test command before expecting contributors to run it.

## Commit & Pull Request Guidelines

Use Conventional Commits: `<type>(<scope>): <short description>`, followed by an optional explanation of what changed and why, and `Refs: NIL-<issue>` when applicable. Use imperative subjects that describe what changed, not what the agent did. Avoid messages such as `Codex changes`, `update files`, `work on milestone`, or `implemented stuff`.

Types: `feat` (new NIL/compiler functionality), `fix` (bug fix), `refactor` (no behavior change), `perf` (performance), `test` (tests), `docs` (documentation/specification), `bench` (benchmarks), `exp` (experimental/research work), and `chore` (tooling/dependencies/CI).

Scopes include `parser`, `syntax`, `types`, `hir`, `mir`, `compiler`, `runtime`, `plugin`, `codegen`, `cli`, `tokenbench`, `tokens`, `spec`, and `ci` for infrastructure. Use `.gitmessage` as the commit template; never invent an issue ID.
Pull requests should explain the purpose, summarize changes, and report validation performed or why it was unavailable. Link relevant issues and include screenshots for visible interface changes. Merge pull requests using **Squash and merge** so each PR lands as one Conventional Commit. Do not use merge commits or rebase merging.

## Security & Configuration

Keep credentials and local environment files out of version control. Provide placeholder configuration examples and ignore generated outputs and dependency directories when establishing the project.
