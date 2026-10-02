# Scalar lazy loop: actual lowered IR

Source: `benchmarks/application/workloads/newlines.nil`.
Before is starting revision `09c30a6`; after is the rootless-region change.
Command: `nil --profile expr-v5 llvm benchmarks/application/workloads/newlines.nil`.
These are complete emitted loop-function definitions, not pseudocode. The C
runtime still implements the bounds/width checks in `nil_get`.

Before: four root slots, nine static stores in the function, **six stores per
continuing iteration** across header/body/backedge. After: two slots, four static
stores, **zero on the continuing path**; roots enter in the preheader and leave
at exit. The arms still branch and join with a phi. Entry/exit roots remain.
The full read-and-loop module has 13 store calls before and 8 after.

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
  %v2 = phi ptr [ %p0, %b0 ], [ %v2, %b6 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v9, %b6 ]
  %v4 = phi i64 [ 0, %b0 ], [ %v13, %b6 ]
  call void @nil_root_store(ptr %v5, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr null)
  %v7 = icmp slt i64 %v3, %v1
  br i1 %v7, label %b2, label %b3
b2:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_root_store(ptr %v8, ptr %v2)
  %v9 = add i64 %v3, 1
  %v10 = call i64 @nil_get(ptr %v2, i64 %v3, i64 43, i64 46)
  %v11 = icmp eq i64 %v10, 10
  br i1 %v11, label %b4, label %b5
b3:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_roots_leave(ptr %nil_root_frame)
  ret i64 %v4
b4:
  br label %b6
b5:
  br label %b6
b6:
  %v12 = phi i64 [ 1, %b4 ], [ 0, %b5 ]
  %v13 = add i64 %v4, %v12
  call void @nil_root_store(ptr %v8, ptr null)
  br label %b1
}
```

## After

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %nil_root_slots = alloca [2 x ptr], align 8
  store [2 x ptr] zeroinitializer, ptr %nil_root_slots, align 8
  %nil_root_frame = call ptr @nil_roots_enter(ptr %nil_root_slots, i64 2)
  %v0 = getelementptr [2 x ptr], ptr %nil_root_slots, i64 0, i64 0
  %v1 = getelementptr [2 x ptr], ptr %nil_root_slots, i64 0, i64 1
  call void @nil_root_store(ptr %v0, ptr %p0)
  call void @nil_root_store(ptr %v0, ptr null)
  call void @nil_root_store(ptr %v1, ptr %p0)
  %v2 = call i64 @nil_length(ptr %p0)
  br label %b1
b1:
  %v3 = phi ptr [ %p0, %b0 ], [ %v3, %b6 ]
  %v4 = phi i64 [ 0, %b0 ], [ %v7, %b6 ]
  %v5 = phi i64 [ 0, %b0 ], [ %v11, %b6 ]
  %v6 = icmp slt i64 %v4, %v2
  br i1 %v6, label %b2, label %b3
b2:
  %v7 = add i64 %v4, 1
  %v8 = call i64 @nil_get(ptr %v3, i64 %v4, i64 43, i64 46)
  %v9 = icmp eq i64 %v8, 10
  br i1 %v9, label %b4, label %b5
b3:
  call void @nil_root_store(ptr %v1, ptr null)
  call void @nil_roots_leave(ptr %nil_root_frame)
  ret i64 %v5
b4:
  br label %b6
b5:
  br label %b6
b6:
  %v10 = phi i64 [ 1, %b4 ], [ 0, %b5 ]
  %v11 = add i64 %v5, %v10
  br label %b1
}
```
