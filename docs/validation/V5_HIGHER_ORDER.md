# Round 13 validation: static HOF

The HOF insertion-sort control remains slow (1,022 versus 2.990 ms at 2,048 elements), and new ex_accumulate source costs about three times Python. Neither result is hidden by the successful implementation.

ADR 034 (`59ec17d`) precedes implementation (`d1148f2`) and tests (`6cf7e48`). Intrinsics remain 35. Static specialization supports caller-local callbacks through module exports without function values or indirect calls in HIR. All earlier profiles/default and six sampled programs' LLVM IR remain unchanged.

Commands completed after implementation: `./scripts/ci.sh` and `./scripts/ci.sh release`: 422 tests each, no clippy warnings; `./scripts/sanitize.sh`: 169 tests, nine clean suites; `./scripts/fuzz-v5.sh --cases 418 --seed SEED`: zero divergences for 5130572, 1729 and 4294967295, 1,254 programs/2,508 native O0/O2 builds including host effects. Full corpus verification passes 2,815 checks; `--group original` separately passes exactly 610.

At 16 MiB, map-reduce HOF/direct medians are 7.186/7.081 ms; noinline HOF/direct 19.945/19.997 ms, over 25 repeats. Materialized maps also match and double-transform byte-exactly. Vectorized scan lowering and genuine noinline calls are backed by IR/disassembly. Historical append/scan/transform medians: 22.709/7.849/36.156 ms; scan's +3.8% historical variation has byte-identical baseline/current native code in a matched audit.

Corpus now contains 68 verified NIL programs and 11 unsupported tasks. Old sources/token counts are unchanged; newly supported accumulate widens external Python density deficit to 26.9–36.4%. Source density is not model-efficiency evidence. Part B deferred with reasons in the full report.

[Full report, commands, per-code counts, min/median/max tables, limitations and receipt links](../../benchmarks/reports/2026-10-06/V5_HIGHER_ORDER.md). Adjacent JSON/CSV artifacts contain raw samples, verification and tokenizer counts; ARCHIVE.json identifies the local raw-log archive.
