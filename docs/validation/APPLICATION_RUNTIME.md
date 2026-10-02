# Expr-v5 checked application runtime: command evidence

Verified locally on 2026-10-02, macOS arm64, Apple Clang 21.0.0. The compiler
sources measured below match their manifest hashes. Scope: bulk reads, checked
LTO accessors, root-count accounting, preserved-length hoisting and conservative
root retention. The 64 MiB quota, E012–E018 diagnostic ordering and earlier/default
profiles are unchanged. C remains the native access-semantics source of truth;
Rust reference evaluation remains an independent differential oracle.

## Required CI and sanitizers

All commands ran from the compiler root and exited 0:

```text
$ ./scripts/ci.sh
188 tests passed across suite result lines; 0 FAILED markers, 0 warnings
$ ./scripts/ci.sh release
188 tests passed across suite result lines; 0 FAILED markers, 0 warnings
```

Totals above are sums of actual `test result: ok. N passed` lines, not one runner's
output line. Debug includes rustfmt, strict Clippy (`-D warnings`) and all-target
builds; release includes optimized builds and doctests. The validation harness
recorded these exit statuses/durations:

```text
EXIT ci 0 seconds 64.42
EXIT release 0 seconds 55.08
EXIT sanitize 0 seconds 41.73
EXIT fuzz-5130572 0 seconds 97.85
EXIT fuzz-8675309 0 seconds 94.15
EXIT fuzz-424242 0 seconds 94.38
```

```text
$ ./scripts/sanitize.sh
running 17 tests
test application_native_matches_reference_at_both_optimization_levels ... ok
test bulk_read_quota_admission_matches_reference_at_exact_boundary ... ok
test byte_construction_diagnostic_priority_matches_reference ... ok
test changing_sequence_length_is_not_hoisted ... ok
test dynamic_ir_keeps_copying_when_original_reads_remain_live ... ok
test dynamic_ir_reuses_proven_dead_replacements ... ok
test dynamic_length_is_hoisted_and_dead_roots_are_cleared_once ... ok
test dynamic_operations_retain_bounded_execution_checks ... ok
test native_buffer_length_is_runtime_sized ... ok
test native_bulk_read_streams_binary_input_through_eof ... ok
test native_dynamic_failures_match_reference_codes ... ok
test native_file_copy_is_binary_exact_and_missing_files_are_diagnosed ... ok
test native_hexadecimal_literals_round_trip_non_utf8_bytes ... ok
test native_live_allocation_quota_and_stdout_effects_are_observable ... ok
test native_typed_input_validation_and_empty_sequences ... ok
test one_mib_file_transform_preserves_every_byte_in_reference_and_native ... ok
test seeded_runtime_buffer_transforms_match_an_independent_oracle ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.67s
exit status: 0
```

Both native IR and C are instrumented. Recovery is disabled; leak detection is
explicitly disabled, so this is not leak-freedom certification. New coverage:
streamed binary reads around 64 KiB growth boundaries, exact read-quota admission,
hoisted lengths, changing-length fallback, known-dead root clears, and a negative
root-retention proof for old reads/calls. The 1 MiB transform test now verifies a
second transform returns the original at O0/O2. Five additional tests raise the
previous 183 count to 188.

## Three seeded oracle campaigns

```text
$ ./scripts/fuzz-v5.sh --cases 192 --seed 5130572 --out fuzz/artifacts/speed-5130572
{"seed":5130572,"programs":192,"native_builds":384,"divergences":0,"observed_codes":{"E012": 20, "E013": 12, "E014": 16, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 92},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

```text
$ ./scripts/fuzz-v5.sh --cases 192 --seed 8675309 --out fuzz/artifacts/speed-8675309
{"seed":8675309,"programs":192,"native_builds":384,"divergences":0,"observed_codes":{"E012": 20, "E013": 12, "E014": 16, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 92},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

```text
$ ./scripts/fuzz-v5.sh --cases 192 --seed 424242 --out fuzz/artifacts/speed-424242
{"seed":424242,"programs":192,"native_builds":384,"divergences":0,"observed_codes":{"E012": 20, "E013": 12, "E014": 16, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 92},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

Together: **576 generated programs, 1,152 native O0/O2 builds, zero divergences**.
Each seed runs four cycles of the 48 families, including ten alias/trap-hostile
families. Results, result/stdout bytes, file effects, and diagnostic codes match
reference evaluation with an in-memory Host. Per-seed observed outcomes:
E012=20, E013=12, E014=16, E015=12, E016=16, E017=12, E018=12, OK=92.
These counts match the starting storage oracle. All nine intrinsics have dedicated
executed scenarios; the JSON intrinsic list itself measures source presence.
This is bounded scenario/compositional generation, not exhaustive or coverage-guided
fuzzing. No generation failure was hidden or narrowed out.

## Complete file transform, capacity and pipe

The existing full-size test verifies every byte in reference/native O0/O2 and an
unchanged input, plus native double-transform round trips. The benchmark also
verifies complete output and round trips at 4 KiB, 1 MiB and 16 MiB.

```text
$ ./target/release/nil --profile expr-v5 build examples/expr-v5/transform_file.nil -o /tmp/nil-dynamic-acceptance/transform -O2
built /tmp/nil-dynamic-acceptance/transform
$ python3 /tmp/nil-dynamic-limit.py
Expected exact payload boundary for these paths: 33554365
size 33554365 status 0 stdout 33554365 stderr  seconds 0.35
Exact inversion verified; output SHA256 2b34b3b2f3151b1f184e9dffde006548dec7f54c5f4ae00d5da4e11c13a684d6
size 33554366 status 1 stdout  stderr E013 @46..58 dynamic allocation limit exceeded seconds 0.008
```

The boundary harness generates the 0..255 byte pattern at each size, executes
`/tmp/nil-dynamic-acceptance/transform INPUT OUTPUT`, compares every inverted byte,
and checks a failure leaves an existing output sentinel untouched. Its exact
boundary expression is `floor((67108864 - 64 - len(output_path) - 32) / 2)` for
`/tmp/nil-dynamic-acceptance/output.bin` (37 bytes). Largest verified input remains
**33,554,365 bytes**; it has not changed. Other live values/path lengths reduce the
capacity; native bounded/reference step guards can fail earlier with E008. The
quota is semantic live/transient storage, not exact RSS.

For the non-regular path, `file_size.nil` was compiled and passed binary stdin
through a subprocess pipe with a 10-second timeout:

```python
payload = bytes(range(256)) * 513 + b"\xff"
p = subprocess.run(["/tmp/nil-speed-read", "/dev/stdin"], input=payload,
                   capture_output=True, timeout=10)
assert p.returncode == 0 and p.stdout == b"131329\n" and not p.stderr
```

Actual check output:

```text
$ ./target/release/nil --profile expr-v5 build examples/expr-v5/file_size.nil -o /tmp/nil-speed-read -O2
built /tmp/nil-speed-read
pipe /dev/stdin: 131329 bytes; stdout: 131329 ; exit: 0 ; no truncation, timeout or stderr
```

The automated pipe test additionally compares complete returned binary bytes at
0, 65,535, 65,536, 65,537 and 131,073 bytes at O0/O2 under sanitizers. Reading a pipe
still requires its writer to close; there is no language I/O timeout guarantee.

## Real benchmark and emitted-code evidence

```text
$ ./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-application-runtime-measured
Benchmark artifacts: /private/tmp/nil-application-runtime-measured
manifest status: passed
```

See [all sizes, before/after and per-stage medians](../../benchmarks/reports/2026-10-02/APPLICATION_RUNTIME.md).
NIL measures 4.484 ms at 1 MiB and 34.617 ms at 16 MiB; both targets are met.
This is within 3× of this C++ baseline, not general C++ parity. The remaining
cost is attributed primarily to scalar checked-loop execution, supported by the
stage comparisons and binary inspection. Separate stage medians are not additive
instrumented timings. The raw archive includes sample arrays, logs and disassembly.

```text
$ otool -tvV /tmp/nil-dynamic-acceptance/transform
... ldrb w8, [x8, #0x20]
... cmp x9, x10
... b.hi ...
... cmp x8, #0x1
... strb w20, [x8, #0x20]
$ otool -tvV /tmp/nil-application-runtime-measured/transform-cpp
... mvn.16b v0, v0
... mvn.16b v1, v1
```

These are instruction excerpts (addresses omitted), not complete tool output.
No nil_get/nil_length/nil_set_unique/nil_root_store symbol/call occurs in the
optimized NIL binary. Source IR still calls the checked helpers; LTO inlines
their C definitions. IR tests assert one length call dominates the loop and
last-use call transfers emit exactly one null clear per function, rather than
re-clearing dead slots at region exit. Runtime alias-hostile coverage validates
the root-count/retention decisions.

## Findings and practical limits

An intermediate application test run failed because the new counter protocol
was initially bypassed by direct driver argument-root writes. The driver now
uses the same root-store primitive, and all final campaigns pass. LTO alone
left accessors out of line; binary inspection led to explicit always-inline
attributes. No sanitizer defect or reference/native divergence was found in the
final runs. These findings are implementation checks, not proof of all possible
programs. Linux LLD installation is wired into CI but local validation was macOS;
no local Linux execution is claimed.

The benchmark facade's seven existing unit tests pass; its new application/stage
path was exercised by the full real benchmark. Documented examples (buffer,
uppercase, parsing, integer file, copy, file size, transform, line count, greet,
sum and native copy) still match their expected output. The earlier add, v3
weighted and v4 reverse fixtures remain byte-for-byte unchanged. No views, new
types, plugin ABI, unsafe profile or changed checks/defaults were introduced.
