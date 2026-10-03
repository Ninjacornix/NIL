# Induction-proved byte loads

Actual unoptimized LLVM for `newlines.nil` after 03c0f49. Data GEP at offset 40
is in the preheader. The original index < length condition remains. The body uses
load i8 / zext, align 1, with no nil_get or per-access failing branch. Pointer
uniqueness and nonempty payload are NOT assumed. Buffer equivalents use align 8.

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %v0 = getelementptr i8, ptr %p0, i64 40
  %v1 = call i64 @nil_length(ptr %p0)
  br label %b1
b1:
  %v2 = phi ptr [ %p0, %b0 ], [ %v2, %b6 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v6, %b6 ]
  %v4 = phi i64 [ 0, %b0 ], [ %v12, %b6 ]
  %v5 = icmp slt i64 %v3, %v1
  br i1 %v5, label %b2, label %b3
b2:
  %v6 = add i64 %v3, 1
  %v7 = getelementptr i8, ptr %v0, i64 %v3
  %v8 = load i8, ptr %v7, align 1
  %v9 = zext i8 %v8 to i64
  %v10 = icmp eq i64 %v9, 10
  br i1 %v10, label %b4, label %b5
b3:
  ret i64 %v4
b4:
  br label %b6
b5:
  br label %b6
b6:
  %v11 = phi i64 [ 1, %b4 ], [ 0, %b5 ]
  %v12 = add i64 %v4, %v11
  br label %b1
}
```

Proof: initial index is a nonnegative constant; the exact condition requires
index < unchanged sequence length; each next index is index + 1. All body
operations are proved allocation/mutation-free. Length is bounded by 64 MiB,
so increments under that condition cannot wrap. Therefore every matching body
access has 0 <= index < length. The condition itself remains the upper-bound
check. There is no new preheader trap that could fire in a zero-trip loop.
Computed starts, shifted indices, other conditions, non-identity sequences and
allocating bodies retain checks. Range facts end before finish: index == length
there must still trap with E012. This boundary has native and fuzz regressions.

With the same O2/LTO pipeline, baseline remarks say call/instruction cannot be
vectorized; after says vectorization width 16, interleaved count 2. The actual
scan binary changes from no byte cmeq SIMD instructions to cmeq.16b and cmeq.8b.
Full remarks and disassembly are retained with the benchmark raw archive. This
is an emission change; LLVM is neither linked, patched nor vendored.

See [ADR 021](../adr/021.md) and the
[measurements](../../benchmarks/reports/2026-10-03/SEQUENCE_RETURNS.md).
