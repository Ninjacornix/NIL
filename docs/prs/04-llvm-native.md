<!-- Title: feat(codegen): execute NIL through LLVM
Head: feat/llvm-native
Base: exp/token-reduction
Merge method: Squash and merge
-->

## Purpose

The reference evaluator validates semantics but does not provide native execution. Add an LLVM ahead-of-time backend and make `nil run` compile and execute native code. Keep the evaluator as a differential oracle and provide `nil build` for reusable executables.

## Changes

- Add `nil-llvm`, consuming validated HIR independently of source syntax.
- Lower structured regions into LLVM SSA blocks and phi nodes, preserving lazy branches and simultaneous loop updates.
- Emit checked arithmetic, division guards, and matching fuel/call-depth accounting.
- Add `llvm` and `build` commands, a generated C entry wrapper, native diagnostics, and explicit toolchain errors.
- Add O0/O2 differential tests, host CI requirements, and separate runtime/build-stage measurements.

## Validation

```text
Complete-stack checks at ab37194:
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.

Recorded benchmark run (not rerun while preparing this description):
uv run --project benchmarks/paired --locked python benchmarks/paired/native.py --output /tmp/native-llvm.json — Python/reference/LLVM O0/O2 agreed on 192 vectors.

Coverage includes CLI build/run integration, missing-Clang errors, compile-fail
and HIR validation suites, deterministic LLVM emission, arithmetic traps,
lazy branches, recursion, parallel/bool loop state, and exact budgets.
No HIR/diagnostic golden fixtures change. Backend SSA lowering is tested;
a stable portable MIR serialization is not introduced.
```

## Issues and compatibility

`nil run` now requires Clang 15+, system C headers, and a linker; there is no interpreter fallback. Generated executables run without Rust or the NIL evaluator. Initial builds target the host on macOS/Linux. Checked arithmetic and existing diagnostic codes/spans are preserved; backend/toolchain failures gain a structured diagnostic.

See [native compilation specification](https://github.com/Ninjacornix/NIL/blob/feat/llvm-native/docs/language/NATIVE_LLVM.md) and [ADR 012](https://github.com/Ninjacornix/NIL/blob/feat/llvm-native/docs/adr/012.md). No linked issue.
