# Scalar-call root traffic

Actual `nil --profile expr-v5 llvm benchmarks/application/workloads/newlines_leaf.nil`
lowering, before `ef6ddf1` and after `812796a`. This is unoptimized emitted LLVM,
not disassembly after inlining. The source scans a file and calls a scalar helper:

```nil
(s)=b(!read(a))
(s)=@(a,0,0;b<#a;a,b+1,c+c(a[b]);c)
1=a==10?1:0
```

The scanner `nil_fn1` changes from four root slots/nine static root stores (six
stores per continuing iteration) to no root frame/stores. The call to `nil_fn2`
remains. Checked `nil_get`, explicit branch edges and phi joins remain. The
allocating file-reading caller `nil_fn0` keeps its four stores. For the
sequence-parameter helper control, `nil_fn2` also changes from two stores and a
root frame to none; the scalar comparison helper already had none.

## Before

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %nil_root_slots = alloca [4 x ptr], align 8
  store [4 x ptr] zeroinitializer, ptr %nil_root_slots, align 8
  %nil_root_frame = call ptr @nil_roots_enter(ptr %nil_root_slots, i64 4)
  %v0 = getelementptr [4 x ptr], ptr %nil_root_slots, i64 0, i64 0
  %v5 = getelementptr [4 x ptr], ptr %nil_root_slots, i64 0, i64 1
  %v6 = getelementptr [4 x ptr], ptr %nil_root_slots, i64 0, i64 2
  %v8 = getelementptr [4 x ptr], ptr %nil_root_slots, i64 0, i64 3
  call void @nil_root_store(ptr %v0, ptr %p0)
  call void @nil_root_store(ptr %v0, ptr null)
  %v1 = call i64 @nil_length(ptr %p0)
  br label %b1
b1:
  %v2 = phi ptr [ %p0, %b0 ], [ %v2, %b2 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v9, %b2 ]
  %v4 = phi i64 [ 0, %b0 ], [ %v12, %b2 ]
  call void @nil_root_store(ptr %v5, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr null)
  %v7 = icmp slt i64 %v3, %v1
  br i1 %v7, label %b2, label %b3
b2:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_root_store(ptr %v8, ptr %v2)
  %v9 = add i64 %v3, 1
  %v10 = call i64 @nil_get(ptr %v2, i64 %v3, i64 44, i64 47)
  %v11 = call i64 @nil_fn2(ptr %ctx, i64 41, i64 48, i64 %v10)
  %v12 = add i64 %v4, %v11
  call void @nil_root_store(ptr %v8, ptr null)
  br label %b1
b3:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_roots_leave(ptr %nil_root_frame)
  ret i64 %v4
}
```

## After

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %v0 = call i64 @nil_length(ptr %p0)
  br label %b1
b1:
  %v1 = phi ptr [ %p0, %b0 ], [ %v1, %b2 ]
  %v2 = phi i64 [ 0, %b0 ], [ %v5, %b2 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v8, %b2 ]
  %v4 = icmp slt i64 %v2, %v0
  br i1 %v4, label %b2, label %b3
b2:
  %v5 = add i64 %v2, 1
  %v6 = call i64 @nil_get(ptr %v1, i64 %v2, i64 44, i64 47)
  %v7 = call i64 @nil_fn2(ptr %ctx, i64 41, i64 48, i64 %v6)
  %v8 = add i64 %v3, %v7
  br label %b1
b3:
  ret i64 %v3
}
```

The proof is in [HIR](../../crates/nil-hir/src/borrowing.rs), not an LLVM
inlining heuristic. Native tests assert the calls survive emission and the root
frames disappear. The benchmark-only noinline control retains a machine-level
helper call; its cost and disassembly evidence are in the
[report](../../benchmarks/reports/2026-10-03/CALL_ROOTS.md).
