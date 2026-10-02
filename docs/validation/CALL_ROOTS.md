# Interprocedural borrowing command evidence

Validated locally on macOS arm64, Rust 1.98.1, Apple Clang 21.0.0.
Compiler and benchmark commits remain local; nothing was pushed.

## Automated checks

```text
./scripts/ci.sh                         exit 0; 206 tests; zero warnings
./scripts/ci.sh release                 exit 0; 206 tests; zero warnings
./scripts/sanitize.sh                   exit 0; 29 native application tests
./scripts/ci.sh clippy                  exit 0; Clippy -D warnings
python3 -m unittest discover -s benchmarks/tests -v
                                       exit 0; 9 tests
```

Logs: `/tmp/nil-call-{ci,release,sanitize,clippy,benchmark-tests}.log`, included
in the local raw archive linked from the measurement report. Debug CI includes
rustfmt/Clippy; both modes retain parser/type/HIR/native/regression/earlier-profile
coverage. Sanitizers instrument emitted LLVM and C, with leak detection disabled
as documented by `sanitize.sh`. The C runtime is unchanged.

## Safety and IR

`borrowing_summary_crosses_calls_and_keeps_allocating_callees_conservative`
checks positive leaf/parse/mutually-recursive summaries, negative sequence returns,
host effects/allocations, and rejection propagating through a recursive group.
`scalar_helpers_and_nested_identity_loops_have_no_root_frame` checks emitted calls
remain, all root frame/stores disappear, and sequence-reading helpers, nested
loops and repeated parse execute correctly at O0/O2.

`borrowing_recursion_keeps_depth_checks_and_sequence_reads` executes self and mutual
recursion with bytes still read after each child call, at depths 0/1/3/253/254/255.
The entry plus helper frames reach the default limit of 256; over-limit recursion
matches reference E008. Explicit `nil_enter` guards remain in bounded IR. Both
optimization levels compare reference results/codes.
`allocating_recursive_frames_and_call_aliases_stay_rooted` verifies recursive
concat frames and caller aliases keep roots, with O0/O2 result comparisons.

Existing lazy tests now include called helpers: unselected checked index never
fires, selected reads execute; unselected called BAD output/division never occurs;
called effects preserve ABCD order; selected called output B precedes E009 and
later D never occurs. A capturing reference Host and O0/O2 compare complete stdout
and diagnostics. Existing direct short-circuit, allocation-after-borrow, alias,
quota, replacement/append, traps and effect tests continue to pass.

Two preexisting IR assertions initially failed because they expected redundant
roots in newly proved whole functions. Execution passed; expectations were updated
to zero roots and extended with conservative negative cases. No reference/native
or sanitizer runtime defect was found. See [actual before/after IR](../architecture/CALL_ROOTS_IR.md).

## Differential oracle

```sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 288 --out /tmp/nil-call-fuzz-5130572
./scripts/fuzz-v5.sh --seed 8675309 --cases 288 --out /tmp/nil-call-fuzz-8675309
./scripts/fuzz-v5.sh --seed 424242 --cases 288 --out /tmp/nil-call-fuzz-424242
```

Every command exits 0: 288 programs, 576 native builds, zero divergences.
Total: **864 programs / 1,728 native builds**, reference versus O0 and O2.
Per-seed executed outcomes are identical:

| Seed | OK | E012 | E013 | E014 | E015 | E016 | E017 | E018 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 5130572 | 188 | 20 | 12 | 16 | 12 | 16 | 12 | 12 |
| 8675309 | 188 | 20 | 12 | 16 | 12 | 16 | 12 | 12 |
| 424242 | 188 | 20 | 12 | 16 | 12 | 16 | 12 | 12 |

Each seed runs four instances of each new family: `leaf_loop`, `sequence_return_loop`,
`calls_lazy`, `recursion_reads`, `mutual_recursion_reads`, `called_effect_lazy`,
`recursive_allocation`, `nested_call_loop`. Earlier 64 families remain, including
append/alias-hostile, selected/unselected effects and nested lazy traps. Generated
syntax includes all nine intrinsics. The oracle compares diagnostic codes and
ordered stdout/file effects even on failure. E009/E008 and near-limit recursion
are exercised by native tests; this campaign's executed outcomes are E012–E018.

This is a seeded scenario/composition generator, not arbitrary AST or coverage-guided
fuzzing. Four randomized repetitions per family/seed are bounded evidence, not a
proof of all programs. Corpus adds call leaf, nested and recursive-live fixtures.
One concurrent launch initially could not find `target/release/v5` while Cargo
all-target builds were replacing it. It was rerun successfully; final campaigns
all completed. Timing runs follow completion of all validation workloads.


## Performance commands

All six final benchmark manifests report `passed` and exit 0. The
[measurement report](../../benchmarks/reports/2026-10-03/CALL_ROOTS.md) includes
commands, exact revisions/source-hash checks, every raw median and the local
archive. Forced-noinline and recursive helper disassembly retain actual `bl
_nil_fn2` calls; the recursive helper calls itself. No inline/pure annotation was
added to NIL. The sequence-returning identity control stays conservative.

Append and transform `llvm` output compares byte-identical to the detached
`ef6ddf1` compiler using `cmp`; hashes remain:

```text
append     fde97ab4d41c82c7f7f762a8f670f21700379d98dbca6c62319bb6013f39e945
transform  632f7d1fc9d70615bd0ced53c9dbb2435424a7b476c6133f59a824f413aff568
```

Benchmark control setup caught a wrong file-versus-size argument classification
for the additional sequence-return control. It was corrected, regression-tested,
and rerun. Filtered results now truthfully mark append content verification false
when append was not selected. Failed/preliminary manifests are not reported as
successful measurements. All final scan shapes additionally count an arbitrary
0..255 byte fixture correctly, outside timing; every final transform is byte-exact
and double-transform round-trips. No sanitizer or differential divergence was found.
