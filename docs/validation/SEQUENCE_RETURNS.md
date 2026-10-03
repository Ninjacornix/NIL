# Sequence returns and induction reads: command evidence

macOS arm64, Apple M2, Rust 1.98.1, Apple Clang 21.0.0.
All compiler and benchmark commits are local; nothing was pushed.

```text
./scripts/ci.sh                    exit 0; 208 tests; zero warnings
./scripts/ci.sh release            exit 0; 208 tests; zero warnings
./scripts/sanitize.sh              exit 0; 31 native application tests
```

Debug includes rustfmt/Clippy -D warnings. Logs are `/tmp/nil-seq-{ci,release,sanitize}.log`.
Reference/native O0/O2, HIR validation, parser failures, diagnostics, regression
and earlier-profile checks remain. No unsafe Rust, crates, LLVM libraries or patches.

New tests verify parameter/selected-alias/recursive sequence returns are rootless,
while slices and conditionally allocating callees remain rooted. A returned alias
crosses into a caller allocating almost 64 MiB before reading the alias and original;
O0/O2 and sanitizers verify neither was collected. Direct byte/buffer induction reads
match expected results, including empty sequences. Computed negative starts,
mismatched bounds, shifted indices and finish reads keep E012 at O0/O2. Existing
lazy called effects, self/mutual/depth-limit recursion, quota/alias/append checks pass.

During review, initial range facts were found to extend into finish, where index
can equal length. The scope was corrected before sanitizer/differential runs;
a native regression and fuzz family explicitly require E012 on that exit read.
Initial ABI assertions also failed C compilation until stddef.h was included.
An old IR test expected roots for an alias-only sequence branch; it was updated
with a real conditionally allocating negative case. These were corrected findings,
not hidden proof exclusions. No sanitizer or differential divergence was found.

## Differential campaigns

```sh
./scripts/fuzz-v5.sh --cases 320 --seed 5130572 --out /tmp/nil-seq-fuzz-5130572
./scripts/fuzz-v5.sh --cases 320 --seed 8675309 --out /tmp/nil-seq-fuzz-8675309
./scripts/fuzz-v5.sh --cases 320 --seed 424242 --out /tmp/nil-seq-fuzz-424242
```

Each exits 0: 320 programs, 640 native builds, zero divergences. Total **960
programs / 1,920 O0/O2 builds**, comparing values, codes and ordered stdout/files
against the reference Host. Executed outcomes per seed:

| Seed | OK | E012 | E013 | E014 | E015 | E016 | E017 | E018 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 5130572 | 208 | 32 | 12 | 16 | 12 | 16 | 12 | 12 |
| 8675309 | 208 | 32 | 12 | 16 | 12 | 16 | 12 | 12 |
| 424242 | 208 | 32 | 12 | 16 | 12 | 16 | 12 | 12 |

Every seed reports four instances of each call/range family:

```text
called_effect_lazy
calls_lazy
computed_start
conditional_sequence_alloc
exit_index
leaf_loop
mutual_recursion_reads
negative_start
negative_zero_trip
nested_call_loop
recursion_reads
recursive_allocation
recursive_sequence_alias
sequence_return_loop
shifted_index
slice_return_alias_live
```

All nine intrinsic names are generated. Existing alias/append/lazy/effect families
remain. This is 80 seeded scenario/composition families, not arbitrary or
coverage-guided fuzzing; successful runs are bounded evidence. E009/E008 and
near-limit recursion are covered by native tests, not these outcome counts.

## IR and vectorization

[Sequence before/after and allocating counterexample](../architecture/SEQUENCE_RETURN_IR.md).
[Induction load IR and exact proof](../architecture/INDUCTION_READ_IR.md).
[ADR 021](../adr/021.md). Root removal precedes inlining. Array lengths, data
immutability, exact SSA induction and body-only dominance justify direct loads;
computed/mismatched ranges retain checked nil_get. No noalias or nonempty-payload
promise is emitted. Alignment is checked against the private C layout.

Clang wrapper flags: `-Rpass=loop-vectorize -Rpass-missed=loop-vectorize
-Rpass-analysis=loop-vectorize`, applied to the same O2/LTO build. Baseline scan
reports call/instruction cannot be vectorized; after reports width 16/interleave 2.
`otool -tvV` confirms cmeq.16b/cmeq.8b in the scan after, absent before. No LLVM
modification is involved. Raw LLVM, remarks, assembly, command logs and measured
samples are archived locally by [the full study](../../benchmarks/reports/2026-10-03/SEQUENCE_RETURNS.md).

## Performance evidence

The four benchmark commands in [the report](../../benchmarks/reports/2026-10-03/SEQUENCE_RETURNS.md)
all exited 0: 25 workload and 3 transform cases before and after. Byte-exact
transform and double-transform round-trips passed at all three sizes.
Append and transform LLVM output compares byte-identical before/after.
Raw command outputs and manifests are archived locally at `/Users/ninjacornix/.local/share/nil/benchmarks/archive/sequence-returns-2026-10-03.zip`,
SHA-256 `8ccc70ace766847dde16928b5245ee6c5d5cb35c48ae9bb92417fd2f6e048d5a`. Nothing was uploaded.
