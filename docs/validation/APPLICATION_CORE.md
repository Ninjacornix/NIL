# Expr-v5 application-core hardening evidence

Verified locally on 2026-10-02, macOS arm64, Apple Clang 21.0.0.
Commands ran from the repository root with the pinned Rust toolchain.
The command excerpts below record actual output; zero exit statuses were observed.
The seeded campaign is reproducible; timings are not performance claims.

## Required checks

```text
$ ./scripts/ci.sh
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.40s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.26s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.44s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.02s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.99s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit status: 0
```

175 passing tests (debug); no skipped tests. Debug also passed rustfmt,
Clippy with `-D warnings`, and all-target builds.

```text
$ ./scripts/ci.sh release
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.35s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.12s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.83s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.99s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.96s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit status: 0
```

175 passing tests (release); no skipped tests. Debug also passed rustfmt,
Clippy with `-D warnings`, and all-target builds.

## Seeded differential campaign

```text
$ ./scripts/fuzz-v5.sh --cases 152 --seed 5130572 --out fuzz/artifacts/v5-final
{"seed":5130572,"programs":152,"native_builds":304,"divergences":0,"observed_codes":{"E012": 16, "E013": 16, "E014": 12, "E015": 12, "E016": 16, "E017": 12, "E018": 12, "OK": 56},"generated_intrinsics":["buffer", "bytes", "concat", "format", "out", "parse", "read", "slice", "write"]}
exit status: 0
```

Each program ran in the reference evaluator and native O0/O2. Comparisons
include exact result/stdout bytes and fixture file contents after both success
and failure. Intrinsic coverage in the summary is source presence; explicit
scenario families execute all nine operations. Error counts are observed results.

This is 38 scenario families with randomized nested expressions, parameters and
fixtures, repeated across four cycles. It includes sequence transport through
calls/branches/loops, alias preservation, byte and integer extremes, empty values,
conflicting invalid inputs and cumulative allocation exhaustion. It is stronger
than a happy-path smoke run, but remains bounded scenario/compositional generation,
not exhaustive or coverage-guided fuzzing. Build subprocesses lack a deadline;
native execution has a ten-second deadline.

## Sanitizers

```text
$ ./scripts/sanitize.sh
running 10 tests
test application_native_matches_reference_at_both_optimization_levels ... ok
test byte_construction_diagnostic_priority_matches_reference ... ok
test dynamic_operations_retain_bounded_execution_checks ... ok
test native_allocation_quota_and_stdout_effects_are_observable ... ok
test native_buffer_length_is_runtime_sized ... ok
test native_dynamic_failures_match_reference_codes ... ok
test native_file_copy_is_binary_exact_and_missing_files_are_diagnosed ... ok
test native_hexadecimal_literals_round_trip_non_utf8_bytes ... ok
test native_typed_input_validation_and_empty_sequences ... ok
test seeded_runtime_buffer_transforms_match_an_independent_oracle ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.43s
exit status: 0
```

The script instruments LLVM code and the C runtime with ASan/UBSan, disables
recovery, and is a mandatory nightly step. Leak checking is disabled explicitly;
this proves neither leak freedom nor absence of all undefined behavior.

## Findings and fixes

The expanded campaign found a real runtime diagnostic divergence:

```text
divergence seed=5130600 mode=28 -O0 expected=E014 native=E013
```

`!bytes(67108864,256)` checked quota before byte range in C. Native construction
now matches the reference: negative length first, byte range next, quota last.
Both implementations have regression tests, plus a checked-in corpus reproducer.

It also exposed a harness budget mismatch, not a compiler semantic bug:

```text
divergence seed=5130608 mode=36 -O0 expected=E008 native=E013
```

The reference's original 100,000-instruction limit stopped the finite cumulative
quota program before allocation failure; native execution was unbounded. The
oracle guard is now 1,000,000 instructions. The quota remains 64 MiB.
Initial harness smoke testing also caught an incorrect helper function identifier;
that generator error was corrected before the seeded comparison. Sanitizers
reported no memory-access or UB errors in their integration-test scope.

## Byte escapes and examples

`native_hexadecimal_literals_round_trip_non_utf8_bytes` verifies that
`!concat("\xFF\x00", "\x80\x7f")` returns bytes `FF 00 80 7F` in both
reference and native O0/O2 execution. Compiler tests also reject malformed hex
escapes. The integration test passed in both CI suites and under sanitizers.

The README and application-example commands were executed against the final
release compiler. File fixtures contained bytes `00 FF 78 0A`; `number.txt`
contained `42` followed by one newline. Exact output, empty stderr, and binary
copy contents were asserted. Buffer output was checked against all 300 elements.

```text
PASS nil run examples/add.nil -> b'42\n'
PASS nil --profile expr-v3 run examples/expr-v3/weighted.nil -> b'33\n'
PASS nil --profile expr-v4 run examples/expr-v4/reverse.nil 0 1 2 3 4 5 6 7 8 -> b'[8,7,6,5,4,3,2,1]\n'
PASS nil --profile expr-v5 run examples/expr-v5/greet.nil 0 World -> b'Hello, World\n'
PASS nil --profile expr-v5 run examples/expr-v5/sum.nil 0 [1,2,3,4] -> b'10\n'
PASS nil --profile expr-v5 run examples/expr-v5/buffer.nil 0 300 -> b'[3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3'
PASS nil --profile expr-v5 run examples/expr-v5/uppercase.nil 0 Hello, NIL! -> b'HELLO, NIL!\n'
PASS nil --profile expr-v5 run examples/expr-v5/parse.nil 0 -42 -> b'-42\n'
PASS nil --profile expr-v5 run examples/expr-v5/line_count.nil 0 a
b
 -> b'2\n'
PASS nil check examples/add.nil
PASS nil hir examples/add.nil
PASS nil llvm examples/add.nil
PASS nil --profile expr-v5 run examples/expr-v5/integer_file.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-v5-examples-4lm_1zrv/number.txt -> b'42\n'
PASS nil --profile expr-v5 run examples/expr-v5/copy.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-v5-examples-4lm_1zrv/input.bin /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-v5-examples-4lm_1zrv/output.bin -> b'4\n'
PASS nil --profile expr-v5 run examples/expr-v5/file_size.nil 0 /var/folders/82/vv914qlx6gq32dyd34vnfg4w0000gn/T/nil-v5-examples-4lm_1zrv/input.bin -> b'4\n'
PASS build and native execution examples/add.nil -> b'42\n'
PASS build and native execution examples/expr-v5/copy.nil -> b'4\n'
All documented commands verified; binary copies exact; buffer output is exactly 300 copies of 3.
```

`git diff c614045 -- examples/add.nil examples/expr-v4/reverse.nil` produced
no output: both earlier examples remain unchanged. No storage reuse, views,
quota changes, new profiles as defaults, widths/floats/records or plugin ABI work
was included. All commits remain local; no push command was run.
