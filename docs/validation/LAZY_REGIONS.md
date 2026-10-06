# Lazy-region command evidence

Validated 2026-10-03 on macOS arm64, Rust 1.98.1, Apple Clang 21.0.0.
Both compiler and benchmark commits are local only; nothing was pushed.

## Automated checks

```text
./scripts/ci.sh                         exit 0; 202 tests; zero warnings
./scripts/ci.sh release                 exit 0; 202 tests; zero warnings
./scripts/sanitize.sh                   exit 0; 25 native application tests
python3 -m unittest discover -s benchmarks/tests -v
                                       exit 0; 7 tests
```

Debug CI includes rustfmt and Clippy `-D warnings`. Both modes cover parser/type/HIR,
reference/native O0/O2, diagnostics, regressions and earlier source profiles.
Sanitizers instrument LLVM and C; leak detection stays disabled. Logs are
`/tmp/nil-lazy-{ci,release,sanitize,benchmark-tests}.log`, retained in the raw archive.

`nested_scalar_lazy_regions_keep_root_traffic_outside_the_loop` verifies nested
arms/conditions, no scalar-function root frame, and only four setup/exit stores
for the loop function. `scalar_lazy_traps_and_short_circuit_stay_on_selected_edges`
checks unselected division/bounds traps and conditional short-circuit forms at
O0/O2; selected failures remain E009/E012. No new logical-operator syntax is added.

`lazy_host_effects_and_failure_order_match_the_reference` checks native O0/O2
against a capturing reference Host: ABC ordering; no BAD from an unselected arm;
ACBCZZZD across a conditional loop; and AB effects before selected E009, with no
later C effect. `lazy_allocations_and_sequence_results_keep_conservative_roots`
checks live captures across allocations and last-use borrows followed by collection.
A sequence-returning conditional retains roots. Tests compare bytes, not UTF-8 text.

The [actual before/after lowered IR](../architecture/SCALAR_LAZY_IR.md) has six
root stores per continuing iteration before and zero after. The full loop function
changes from four slots/nine static stores to two slots/four stores. Explicit
branches, selected-edge instructions, phi joins and nil_get checks remain.
No semantic branch capture or budget tick is dropped.

The transform's emitted LLVM IR is byte-identical before/after, SHA-256
`632f7d1fc9d70615bd0ced53c9dbb2435424a7b476c6133f59a824f413aff568`;
the C runtime is unchanged. Append IR is also byte-identical, SHA-256
`fde97ab4d41c82c7f7f762a8f670f21700379d98dbca6c62319bb6013f39e945`.
The conservative scalar-callee scan IR remains byte-identical, SHA-256
`7da31b87c09e9b8d359018618ddcb8a4428ec9bf8b075f814c7c7b5760b1db0f`.
Commands are the same profile/llvm command above
using each compiler and `examples/expr-v5/transform_file.nil`.

## Differential campaigns

```sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 256
./scripts/fuzz-v5.sh --seed 8675309 --cases 256
./scripts/fuzz-v5.sh --seed 424242 --cases 256
```

All commands exit 0: **768 programs, 1,536 native builds, zero divergences**.
Each seed records the same executed outcome counts:

| Seed | Programs | Native builds | E012 | E013 | E014 | E015 | E016 | E017 | E018 | OK |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 5130572 | 256 | 512 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 156 |
| 8675309 | 256 | 512 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 156 |
| 424242 | 256 | 512 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 156 |

Actual summaries include `"divergences":0`; source coverage includes all nine
intrinsics. The seven failure-code counts match the starting four-cycle campaign;
eight additional lazy families increase OK from 124 to 156. Logs are
`/tmp/nil-lazy-fuzz-SEED.log`.

The 64-family campaign compares the independent Rust evaluator/in-memory Host
against native O0/O2, including returned bytes, diagnostic codes, stdout effects
and filesystem contents. Eight new families cover nested scalar arms/conditions,
unselected traps, ordered effects, allocating siblings, old reads after updates,
last-use captures followed by allocation and conservative nested loops. Original
lazy/effect/alias/append/error families remain. The quota and diagnostic priorities
are unchanged. This is bounded scenario/compositional generation, not arbitrary
program or coverage-guided fuzzing. Source intrinsic coverage is not branch coverage.

## Proof boundary and findings

The recursive HIR proof allows scalar arithmetic/comparison/length/index and
qualifying nested Ifs. Calls and intrinsics have no effect/allocation summary;
sequence returns escape the scalar region; nested loops are not covered. These
keep the original conservative protocol. Some particular implementations are
nonallocating, but this proof does not assume that implicitly. The scalar-callee
scan control measures this remaining boundary. Effects and allocating arms are
never made eager to meet the target. No existing single-use proof was narrowed. The initial placement also borrowed
non-lazy condition/finish regions of unproved allocating loops; append medians
rose 8–9%. IR inspection shows the append change was removal of two finish-region
stores and one slot, outside the continuing path. This perturbed code generation;
no counter profile attributes the slowdown. Final placement borrows lazy edges
and proved retained-loop conditions, preserving other unproved-region protocols.
Final append and transform IR are byte-identical to their starting versions.
The prototype measurements are retained and reported; correctness tests had passed
before this placement adjustment. Final validation was rerun in full.

Initial new tests used unsupported &&/|| spelling and E003 instead of the actual
E009 division diagnostic. Tests were corrected to existing conditional forms and
codes; no compiler semantics changed. A preliminary timing oracle assumed the
wrong newline count for the fixture; final measurements use independent byte
counting. Neither sanitizers nor the seeded campaigns found a runtime defect.

Provenance review found that the previous APPLICATION_BUILDERS report's supposed
matched starting-source run was actually another new-version run: bench.sh resets
NIL_ROOT. Its recorded revision is 97b9971, not ac92743. Labels in that historical
report/evidence were corrected; raw samples remain unchanged. This goal invokes
the facade directly with the same Python for the before checkout and verifies
revision/source hashes. No benchmark samples are silently discarded.

## Final performance and reproducibility checks

```text
bash /tmp/nil-lazy-final-measure.sh       exit 0; four manifests passed
Final compiler revisions, same Python and all source hashes verified
```

The [report](../../benchmarks/reports/2026-10-03/LAZY_REGIONS.md) lists the four
exact facade commands. Before manifests record 09c30a6; final after records
a747b46; every run uses Python 3.12.11. Before is invoked directly with NIL_ROOT
and the same interpreter, avoiding the wrapper's intentional checkout override.

16 MiB newline count: 160.733 → **12.310 ms**; C++ 10.230, Python 24.265 ms.
Branchless byte-sum: 10.473 → **10.300 ms** on the same fixture. Separate-median
conditional overhead is **8.956 → 0.120 ns/iteration**; this is not instrumented
instruction timing. The scalar-callee boundary remains **161.036 ms**.

Append medians at 10k / 40k / 160k / 640k / 1 MiB are
**2.793 / 3.532 / 5.995 / 15.933 / 24.552 ms**, retaining linear scaling.
Before 1 MiB is 24.615 ms. Transform medians at 4 KiB / 1 MiB / 16 MiB are
**2.642 / 4.749 / 35.010 ms**, versus 2.577 / 4.832 / 35.144 before.
Small nominal increases remain in the full table; startup/noise is not isolated.
Both emitted programs and the runtime are unchanged. All outputs are verified,
including append contents and byte-exact double-transform round trips.

See [every workload and control](../../benchmarks/reports/2026-10-03/LAZY_REGIONS.md)
for real timings, ratios, remaining losses, raw provenance and limitations.
