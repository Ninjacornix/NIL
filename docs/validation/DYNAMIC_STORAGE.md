# Expr-v5 live storage and reuse: verification

Verified locally on 2026-10-02, macOS arm64, Apple Clang 21.0.0. Commands ran
from the repository root. All command excerpts below are actual output. The quota
meaning intentionally changed from cumulative to live/transient storage; the
limit remains 64 MiB. Earlier profiles and the default remain unchanged.

## Build, lint and tests

```text
$ ./scripts/ci.sh
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.13s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.36s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.95s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.67s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.06s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.60s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit status: 0
```

183 passing tests. Debug also passed rustfmt, strict Clippy (`-D warnings`)
and all-target builds; release passed optimized builds and doctests.

```text
$ ./scripts/ci.sh release
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.05s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.52s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.43s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.38s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.33s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit status: 0
```

183 passing tests. Debug also passed rustfmt, strict Clippy (`-D warnings`)
and all-target builds; release passed optimized builds and doctests.

## Sanitizers

```text
$ ./scripts/sanitize.sh
running 13 tests
test application_native_matches_reference_at_both_optimization_levels ... ok
test byte_construction_diagnostic_priority_matches_reference ... ok
test dynamic_ir_keeps_copying_when_original_reads_remain_live ... ok
test dynamic_ir_reuses_proven_dead_replacements ... ok
test dynamic_operations_retain_bounded_execution_checks ... ok
test native_buffer_length_is_runtime_sized ... ok
test native_dynamic_failures_match_reference_codes ... ok
test native_file_copy_is_binary_exact_and_missing_files_are_diagnosed ... ok
test native_hexadecimal_literals_round_trip_non_utf8_bytes ... ok
test native_live_allocation_quota_and_stdout_effects_are_observable ... ok
test native_typed_input_validation_and_empty_sequences ... ok
test one_mib_file_transform_preserves_every_byte_in_reference_and_native ... ok
test seeded_runtime_buffer_transforms_match_an_independent_oracle ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.07s
exit status: 0
```

ASan/UBSan instrument both emitted LLVM and the C runtime. Recovery is disabled.
Leak checking remains disabled; this is not a leak-freedom certification.

## Two differential campaigns

```text
$ ./scripts/fuzz-v5.sh --cases 192 --seed 5130572 --out fuzz/artifacts/storage-final-a
{"seed":5130572,"programs":192,"native_builds":384,"divergences":0,"observed_codes":{"E012": 20, "E013": 12, "E014": 16, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 92},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

```text
$ ./scripts/fuzz-v5.sh --cases 192 --seed 8675309 --out fuzz/artifacts/storage-final-b
{"seed":8675309,"programs":192,"native_builds":384,"divergences":0,"observed_codes":{"E012": 20, "E013": 12, "E014": 16, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 92},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

Together: 384 programs and 768 native O0/O2 builds. Each comparison uses the
reference evaluator with an in-memory Host and checks exact result/stdout bytes,
diagnostic codes and fixture files, including side effects before later failure.
Each seed executes four cycles of 48 families; ten new families stress retained
originals, shared arguments, captured loop state, lazy arms, returned aliases and
replacement traps. The generator also composes randomized nested sequence expressions.
This is bounded scenario/compositional generation, not coverage-guided fuzzing or
an exhaustive proof. Intrinsic coverage in JSON is source presence; dedicated
families execute all nine operations. Code counts are observed outcomes.

## IR path selection

The source assertions in `crates/nil-llvm/tests/application.rs` are:

```rust
// dynamic_ir_reuses_proven_dead_replacements
assert!(ir.contains("call ptr @nil_set_unique("));
assert!(!ir.contains("call ptr @nil_set("));
// dynamic_ir_keeps_copying_when_original_reads_remain_live
assert!(ir.contains("call ptr @nil_set("));
assert!(!ir.contains("call ptr @nil_set_unique("));
```

These tests passed in debug/release and under sanitizers. CLI IR inspection of
`(s):s=@(a,0;b<#a;a[b:255-a[b]],b+1;a)` and the old-read case
`(s)=@(a,0,0;b<#a;a[b:255],b+1,c+a[b];c)` produced:

```text
$ ./target/release/nil --profile expr-v5 llvm /tmp/nil-storage-safe.nil
%v10 = call ptr @nil_set_unique(ptr %v1, i64 %v2, i64 %v9, i64 18, i64 30)
$ ./target/release/nil --profile expr-v5 llvm /tmp/nil-storage-aliased.nil
%v9 = call ptr @nil_set(ptr %v1, i64 %v2, i64 255, i64 18, i64 25)
```

The excerpt includes replacement calls only. The unique helper still copies if
runtime roots reveal an alias in another state slot or suspended caller. Static
last-use checks prevent immediate mutation when old reads occur later. This
explicitly strengthens the fixed-array proof rather than silently applying its
deferred-write reasoning to immediate dynamic mutation.

## Complete file transform and exact boundary

`examples/expr-v5/transform_file.nil` reads, inverts each byte and writes the result.
The input fixture for the 1 MiB acceptance run was `bytes(range(256)) * 4096`.
Every byte was compared with `255 - input_byte`.

```text
$ ./target/release/nil --profile expr-v5 build examples/expr-v5/transform_file.nil -o /tmp/nil-dynamic-acceptance/transform -O2
built /tmp/nil-dynamic-acceptance/transform
$ /tmp/nil-dynamic-acceptance/transform /tmp/nil-dynamic-acceptance/input.bin /tmp/nil-dynamic-acceptance/output.bin
1048576
Exact byte inversion verified: 1048576 bytes; SHA256 eaeaa7acca0afcaee85d7abae4d8e5033652991ea19df161cc90ceec2803342c
```

The integration test also runs this full fixture in the reference evaluator
(25,000,000-step guard) and native O0/O2, verifying unchanged input and all output
bytes. For the exact path-dependent quota boundary, fixture generation used the
same 0..255 pattern truncated to each size. The binary was invoked as above:

```text
Expected exact payload boundary for these paths: 33554365
size 33554365 status 0 stdout 33554365 stderr  seconds 1.753
Exact inversion verified; output SHA256 2b34b3b2f3151b1f184e9dffde006548dec7f54c5f4ae00d5da4e11c13a684d6
size 33554366 status 1 stdout  stderr E013 @46..58 dynamic allocation limit exceeded seconds 1.045
```

Largest verified successful input: **33,554,365 bytes**. One byte more failed
with E013 before writing; the output sentinel remained untouched. At replacement,
the budget includes old/result payload + 32 each, plus destination-path bytes + 32.
For these paths that gives `floor((67108864 - 64 - 37 - 32) / 2)` bytes. Other live
values or aliases reduce capacity. Native v5 is unbounded by default; enabling
`--bounded`, or retaining a smaller reference instruction guard, can fail earlier
with E008. OS resources can also limit a run. The budget is not exact RSS.

## Real cross-language measurement

```text
$ ./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-application-storage-measured
Benchmark artifacts: /private/tmp/nil-application-storage-measured
manifest status: passed
```

Median whole-process milliseconds (nine samples, one warmup):

| File bytes | NIL | C++ | Python |
|---:|---:|---:|---:|
| 4,096 | 2.683 | 2.463 | 14.397 |
| 1,048,576 | 57.498 | 3.184 | 64.119 |
| 16,777,216 | 887.161 | 12.182 | 760.782 |

NIL is **not at C++ speed**; at 16 MiB it is also slower than this explicit Python
loop. This establishes feasibility and these particular local whole-process results,
not kernel-only speed, production ranking, tokenizer efficiency or LLM TCR.
Timing includes startup, cached read/write, transform, stdout and exit; compilation
and correctness verification are excluded. No fsync, CPU isolation or confidence
interval was used. See the [benchmark report](../../benchmarks/reports/2026-10-02/APPLICATION_STORAGE.md)
for source hashes, all method limits and the external raw archive SHA-256.
The measured C runtime and LLVM emitter hashes match the committed implementation;
the later Rust argument-admission fix does not affect the timed native code.

The benchmark facade's seven existing unit tests also passed:

```text
$ python3 -m unittest discover -s benchmarks/tests -v
Ran 7 tests in 0.146s

OK
```

## Findings and scope

No reference/native divergence or sanitizer memory/UB defect was found in the final
runs. An audit found that the initial live-accounting implementation still charged
aliased reference-entry arguments twice on admission. It was fixed with a regression
using two handles to one 34 MiB allocation. Dead-allocation tests now expect success;
simultaneously live allocation tests still require E013. Development plumbing errors
(a missing LLVM helper declaration and an incorrect benchmark CLI flag) were corrected
before final verification; they were not evidence of an unresolved alias proof.

Slices/concat still copy. No views, ownership syntax, new types, plugin ABI, recoverable
I/O, earlier-profile semantics, default-profile changes or limit increase were added.
The proof remains conservative and copying remains the fallback. No unsound case
was hidden by restricting the generator. Root scans, runtime calls and file-reading
strategy remain performance concerns rather than proven bottlenecks here.

## Documented examples

The README and example guide commands were checked against the final release compiler.
Fixtures used bytes `00 FF 78 0A` and integer text `42` plus a newline. Full stdout,
empty stderr, binary copies and inversion were verified. Buffer output matched all
300 copies of 3. Output excerpts:

```text
PASS nil run examples/add.nil -> b'42\n'
PASS nil --profile expr-v3 run examples/expr-v3/weighted.nil -> b'33\n'
PASS nil --profile expr-v4 run examples/expr-v4/reverse.nil 0 1 2 3 4 5 6 7 8 -> b'[8,7,6,5,4,3,2,1]\n'
PASS nil --profile expr-v5 run examples/expr-v5/greet.nil 0 World -> b'Hello, World\n'
PASS nil --profile expr-v5 run examples/expr-v5/sum.nil 0 [1,2,3,4] -> b'10\n'
PASS nil --profile expr-v5 run examples/expr-v5/buffer.nil 0 300 -> b'[3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,'
PASS nil --profile expr-v5 run examples/expr-v5/uppercase.nil 0 Hello, NIL! -> b'HELLO, NIL!\n'
PASS nil --profile expr-v5 run examples/expr-v5/parse.nil 0 -42 -> b'-42\n'
PASS nil --profile expr-v5 run examples/expr-v5/line_count.nil 0 a
b
 -> b'2\n'
PASS nil check examples/add.nil
PASS nil hir examples/add.nil
PASS nil llvm examples/add.nil
PASS nil --profile expr-v5 run examples/expr-v5/integer_file.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/number.txt -> b'42\n'
PASS nil --profile expr-v5 run examples/expr-v5/copy.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/input.bin /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/output.bin -> b'4\n'
PASS nil --profile expr-v5 run examples/expr-v5/file_size.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/input.bin -> b'4\n'
PASS nil --profile expr-v5 run examples/expr-v5/transform_file.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/input.bin /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-storage-examples-b2gykibq/output.bin -> b'4\n'
PASS build and native execution examples/add.nil b'42\n'
PASS build and native execution examples/expr-v5/copy.nil b'4\n'
18 documented command checks passed; complete result bytes and file copies verified.
```

`git diff 04452d6 -- examples/add.nil examples/expr-v3/weighted.nil examples/expr-v4/reverse.nil`
produced no output. All three earlier examples are unchanged. Compiler and benchmark
changes were committed separately using Conventional Commits. Neither repository
was pushed during this workstream.
