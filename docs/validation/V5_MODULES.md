# Expr-v5 modules: final validation

[ADR 032](../adr/032.md) was committed before implementation. The unified source
loader accepts `nil-module 1`; no new grammar, type, intrinsic or runtime was
added. Imported exports lower to ordinary HIR calls, preserving the existing
proofs. Borrow-only plugins keep their restrictions and accounting. Module
allocation/effects/transitive imports/local recursion work; import cycles and
module-owned nominal registries remain deliberately unsupported.

## Reproduce

From NIL root, with Rust and Clang 15+:

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
./scripts/fuzz-v5.sh --seed 1729 --cases 356 --out /tmp/module-fuzz-1729
./scripts/fuzz-v5.sh --seed 8675309 --cases 356 --out /tmp/module-fuzz-8675309
./scripts/fuzz-v5.sh --seed 5130572 --cases 356 --out /tmp/module-fuzz-5130572
```

Observed final output: **385 tests** debug, **385 tests** release, no clippy
warnings; **150 tests** across all seven sanitizer suites, no finding. Every fuzz
run reports `programs:356`, `native_builds:712`, `divergences:0`: **1068 programs,
2136 native builds** overall. All 24 module execution families and nine graph
mutation families ran once per seed. Static E024 mutations are separate from
native counts. Exact per-code/family counts and log hashes:
[validation receipt](../../benchmarks/reports/2026-10-05/v5-modules-validation.json).

The new compiler tests cover private entries/colliding names, transitive shared
imports, export/version/path/cycle failures, signature/registry checking,
snapshots, ordinary recursion limits and source/graph/function bounds. Native
module tests compare evaluator/O0/O2 outputs, diagnostics and host effects;
allocations, returned aliases, root quotas, recursive frames and laziness all
pass. A borrowed scan emits no root frame before inlining, while an allocating
counterexample keeps root machinery. Existing plugin tests pass unchanged.

Benchmark/corpus environment is optional, in the initialized submodule; no Rust
dependency was added. These commands use the existing verification harness:

```sh
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/module-corpus-all.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/module-corpus-original.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/modules/verify.py --output /tmp/module-corpus-split.json
benchmarks/paired/.venv/bin/python benchmarks/module-study/split_tokens.py --output /tmp/module-token-cost.json
./scripts/bench.sh runtime --full --workloads --workload append --workload newlines --repeats 25 --output /tmp/module-controls
./scripts/bench.sh runtime --full --application --repeats 25 --output /tmp/module-transform
```

Full corpus: **2775 checks passed**, 66 NIL programs, 13 unsupported. Original
610 passed separately. Two alternate existing solutions passed **30 additional
checks** against unchanged inputs/goldens. They share reporting code; their
complete source/manifest bundle adds **33–39 tokens**, of which imported reference
spelling accounts for **8–10**. No small-bundle density advantage is claimed.
The preregistered corpus naming study and all four tokenizer identities/counts
are committed in `benchmarks/module-study/`.

All 264 frozen task/oracle/golden/source files match benchmark revision `2517978`.
All 66 original single-file NIL sources plus the three performance controls emit
**byte-identical before/after LLVM IR**. Runtime/backend/HIR source and dependency
manifests are unchanged. The documented module example outputs were actually run:
CLI `2`, file `2\n`; CLI `3`, file `42\n`.

25-repeat isolated medians: append 1 MiB **22.899 ms**, scan 16 MiB **7.728 ms**,
transform 16 MiB **36.819 ms**. Previous report values were 22.851 / 7.599 / 36.670
ms; the small differences fall within current sample ranges. The report distinguishes
historical values from this session's runs, states the warm whole-process method,
and includes C++/Python, min/max and raw samples. No architectural regression is
observed; noisy finite timing is not a universal speed guarantee.

## Honest boundaries

`multi_file_fixture` is the upstream grep task's **runtime file fixture** gap,
not a source-module requirement. That fixture/adapter remains absent, so modules
do not change the unsupported count. No task, oracle, golden or baseline changed.
The [report](../../benchmarks/reports/2026-10-05/V5_MODULES.md) lists all remaining
blockers, primitive classification, actual token costs, measured controls and
conservative allocation/effect cases. Import cycles, root-owned record registry,
local-offset diagnostics and unstripped imported functions are accepted prototype
limits. No commutative bulk matcher repair or additional capability was attempted.

Review caught and fixed accidental private-label access before final validation.
An early boundary fixture incorrectly expected rejection at the exact legal
limit; it was corrected to overflow by one byte, then both full CI modes reran.
No fuzzer divergence or sanitizer finding occurred. The final tests do not rely
on the superseded 384-test development gates.

## Evidence and local commits

The complete [benchmark report](../../benchmarks/reports/2026-10-05/V5_MODULES.md)
links JSON receipts, tokenizer results, corpus verification and raw timing samples.
Original command logs, including superseded/failed development checks, are archived:

```text
/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-modules-2026-10-05.tar.gz
SHA256 948c6443eb3250fd028f72c86ff0981df738794e51f53a213d5bf5f7495d4bff
```

Final gate logs inside: `ci-debug-final.log`, `ci-release-final.log`, `sanitize.log`.
The three `fuzz-SEED.log` files contain actual complete JSON counts. The archive
contains no model outputs or invented timings. Source implementation commits:
`b1946a6` (decision), `65c972f` (loader), `7f4c58a` (CLI/examples), `1d25537`
(native/fuzz), `f6ef1f0` (optional corpus pin). Benchmark commits `8b3e0e2` and
`d2ea3fb` hold preregistration and reusable verification.

**Everything is committed locally only; nothing was pushed.** Remote tracking refs
remain main `ac92743caa4792413c9a7fa29e1395586e919939`, benchmark
`57bf135495075ad631daa14421f797a1f0f3f370`. No permission or follow-up capability
is required to review this result.
