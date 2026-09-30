<!-- Title: test(compiler): fuzz expr-v3 and license releases
Head: test/expr-v3-fuzz
Base: exp/expr-v3
Merge method: Squash and merge
-->

## Purpose

Exercise the expr-v3 pipeline beyond hand-written examples and retain reproducible inputs when parsing, validation, or native execution fails. Add seeded property/mutation testing with an independent semantic oracle, nightly campaigns, and clear project documentation and licensing.

## Changes

- Add the dependency-free `nil-fuzz` workspace tool, typed generators, byte/source mutations, and HIR mutations.
- Compare the tree oracle, HIR evaluator, and native O0/O2 results in bounded/unbounded modes.
- Isolate native workers with deadlines and preserve source, seed, mode, IR, and logs for replay.
- Add a small checked-in corpus, regression-oriented property tests, nightly artifacts, and a recorded campaign.
- Rewrite the README around the project; add MIT licensing to workspace packages and release archives, with archive-notice verification.

## Validation

```text
Validation of the complete stack at ab37194:
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.
./scripts/package-macos.sh aarch64-apple-darwin — passed; extracted LICENSE matched the repository and the packaged sample returned 42.
/tmp/nil-actionlint/actionlint .github/workflows/release.yml — passed.
README links and all five Cargo package license fields — verified.

Recorded implementation validation:
./scripts/ci.sh release — passed 121 tests.
./scripts/fuzz.sh --cases 20000 --native 64 --seed 5130572 --out /tmp/nil-fuzz-campaign — passed 20,000 generated programs, 100,000 mutated/raw inputs, 20,000 HIR mutations, 768 native builds, and 2,816 native executions.

Coverage includes malformed input, structured diagnostics, type checking,
independent HIR validation, nested/parallel loops, recursive/helper entries,
backend differential execution, worker timeout/reaping, replay, and failure artifacts.
No HIR/diagnostic golden fixtures change. Invalid UTF-8 is rejected before the
string API. Intel packaging/publication was not run locally.
```

## Issues and compatibility

None for NIL grammar or execution semantics. The fuzz tool is additive and does not provide coverage-guided or exhaustive verification. The repository and distributed compiler notices now specify MIT licensing. The campaign report records source hashes and a dirty-tree measurement context; it does not claim to measure the later documentation-only commit.

See [fuzzing methodology](https://github.com/Ninjacornix/NIL/blob/test/expr-v3-fuzz/docs/FUZZING.md) and [campaign report](https://github.com/Ninjacornix/NIL/blob/test/expr-v3-fuzz/fuzz/reports/2026-09-30.json). No linked issue.
