# Incremental application builders: command evidence

Validated 2026-10-02 on macOS arm64, Rust 1.98.1 and Apple Clang 21.0.0.
Changes are on `feat/application-core`; benchmark changes are local to
`exp/application-storage`. **Nothing was pushed.** The pinned benchmark commit is
unpublished and must be published separately by the overseer before a remote PR
can reproduce its gitlink.

## Correctness gates

```text
./scripts/ci.sh                         exit 0; 197 tests; zero warnings
./scripts/ci.sh release                 exit 0; 197 tests; zero warnings
./scripts/sanitize.sh                   exit 0; 21 native application tests
python3 -m unittest discover -s benchmarks/tests -v
                                       exit 0; 7 tests
```

CI includes parser/type/HIR, reference, native O0/O2, diagnostic, regression and
prior-profile coverage. Debug CI enforces rustfmt and Clippy `-D warnings`.
Sanitizers instrument both LLVM and C with ASan/UBSan; leak detection stays disabled.
Logs are `/tmp/nil-append-{ci,release,sanitize,benchmark-tests}.log` and retained
in the local raw archive linked from the benchmark report.

IR integration test `append_ir_extends_dead_operands_and_copies_for_old_reads`
asserts a dead append loop calls `nil_concat_unique` and has no ordinary concat
call, while a later read of the original emits copying `nil_concat`. Other tests
verify reallocations, non-UTF-8 contents, self-concat, retained callers, exact 1 MiB
contents at O0/O2, capacity-sensitive E013, and external Rust argument aliases.

## Differential oracle

```sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 224
./scripts/fuzz-v5.sh --seed 8675309 --cases 224
./scripts/fuzz-v5.sh --seed 424242 --cases 224
```

All three commands exit 0: **672 programs, 1,344 native builds, zero divergences**.
Each seed reports the same observed outcomes:

| Seed | Programs | O0/O2 builds | E012 | E013 | E014 | E015 | E016 | E017 | E018 | OK |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 5130572 | 224 | 448 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 124 |
| 8675309 | 224 | 448 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 124 |
| 424242 | 224 | 448 | 20 | 12 | 16 | 12 | 16 | 12 | 12 | 124 |

Actual summary field: `"divergences":0`; generated intrinsics are buffer, bytes,
concat, format, out, parse, read, slice and write. Per-seed failure counts match
four cycles of the starting failure families; eight new successful append families
increase OK from 92 to 124 per seed. Logs are `/tmp/nil-append-fuzz-SEED.log`.

Each program runs in the independent Rust evaluator with an in-memory Host and in
native O0/O2. Comparisons include returned bytes, diagnostic codes, ordered stdout
and filesystem effects. Each seed cycles all 56 scenario families four times.
Eight append families exercise retained aliases before further appends, lazy
identity arms, post-loop originals, callee arguments retained by callers, mixed
concat/slice/replace chains, buffers, self-concat and formatted reports.

This is bounded scenario/compositional testing, not arbitrary-program or
coverage-guided fuzzing. Source operation coverage is not branch coverage.
Dedicated cases execute all nine intrinsics. O0/O2 share the C runtime, making the
separate Rust oracle essential. Quota boundary changes are intentional; other
codes and diagnostic priority remain unchanged.

## Exact quota boundaries

The canonical append source is `benchmarks/application/workloads/append.nil`.

```sh
./target/release/nil --profile expr-v5 build benchmarks/application/workloads/append.nil -o /tmp/nil-append-boundary -O2
/tmp/nil-append-boundary 33554372
# 33554372; exit 0
/tmp/nil-append-boundary 33554373
# E013 @24..25 dynamic allocation limit exceeded; exit 1
```

`python3 /tmp/nil-append-boundaries.py` builds the documented file transform and
uses `/tmp/nil-dynamic-acceptance/input.bin` and `output.bin`:

```text
size 33554353 status 0 stdout 33554353 stderr
Exact inversion verified; output SHA256
2367bd0e84a6ca1e74da8b1a6f25b2d8ad3078c23a4f89c408cda13bec5c0cac
size 33554354 status 1 stdout [empty]
stderr E013 @46..58 dynamic allocation limit exceeded
Transform and append boundaries verified
```

The successful fixture repeats bytes 0..255, truncated to the tested size. Failure
leaves a prewritten output sentinel unchanged. The transform bound is
`floor((67108864 - 2*40 - (len(output_path_bytes)+40))/2)`;
this output path has 37 bytes. Different live values/paths can reduce it.
The former boundary was 33,554,365: the larger header lowers it by 12 bytes.
Append growth is clamped to a steady capacity of 33,554,371, then one exact-capacity
append fits; the next old+right+result reservation exceeds the limit. Bounded
execution can independently stop earlier with E008. These are workload-specific
limits, not universal maximum allocations.

## Examples and findings

`python3 /tmp/nil-append-examples.py` verifies every documented output in README
and the v5 example guide, including integer lists, greetings, uppercase, decimal
file parsing, byte copy/size/transform, and documented native commands.
Output: `All documented example outputs and binary files verified`.
The default add example, expr-v3 weighted example and expr-v4 reverse example have
no diff from starting revision `ac92743`.

Code review found a temporary evaluator bug charging a HIR literal Vec's incidental
spare capacity. Literals publish exact capacity; the fix charges length and adds
`literal_admission_ignores_hir_vec_spare_capacity`. The seeded campaigns did not
find this bug. An initial exact-read boundary test assumed the old 32-byte header;
its expectation was corrected for the intentional 40-byte accounting change.
No single-use proof was narrowed to avoid a failing case. Aliases and self-concat
retain the conservative copying fallback. Sanitizers found no invalid access/UB.

The transform recheck measures 2.615 / 4.793 / 35.528 ms at 4 KiB / 1 MiB /
16 MiB. Matched starting-source medians are 2.688 / 4.668 / 34.849 ms.
The two larger new medians are nominally 2.7% / 1.9% higher; their sample ranges
overlap. **Strict non-regression is unproven**, and no statistical isolation
attributes this small difference to code versus noise. This limitation is retained
rather than asserting unchanged performance. The prior runtime targets remain met.

See [all measured workloads and limitations](../../benchmarks/reports/2026-10-02/APPLICATION_BUILDERS.md).
