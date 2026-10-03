# Sequence-return control lowering

Actual unoptimized LLVM for the parameter-returning scan, before e922bb4 and
after 03c0f49. Root stores across the module drop from 17 to 4; only the allocating
file-reading caller retains them. The identity helper and loop keep no root
frames. The slice control still has 19 stores: slice copies/allocates in v5.
The callee calls remain in emitted IR; root removal precedes LLVM inlining.

## alias-before

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %nil_root_slots = alloca [5 x ptr], align 8
  store [5 x ptr] zeroinitializer, ptr %nil_root_slots, align 8
  %nil_root_frame = call ptr @nil_roots_enter(ptr %nil_root_slots, i64 5)
  %v0 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 0
  %v4 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 1
  %v5 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 2
  %v8 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 3
  %v10 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 4
  call void @nil_root_store(ptr %v0, ptr %p0)
  call void @nil_root_store(ptr %v0, ptr null)
  br label %b1
b1:
  %v1 = phi ptr [ %p0, %b0 ], [ %v9, %b2 ]
  %v2 = phi i64 [ 0, %b0 ], [ %v11, %b2 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v14, %b2 ]
  call void @nil_root_store(ptr %v4, ptr %v1)
  call void @nil_root_store(ptr %v5, ptr %v1)
  %v6 = call i64 @nil_length(ptr %v1)
  call void @nil_root_store(ptr %v5, ptr null)
  %v7 = icmp slt i64 %v2, %v6
  br i1 %v7, label %b2, label %b3
b2:
  call void @nil_root_store(ptr %v4, ptr null)
  call void @nil_root_store(ptr %v8, ptr %v1)
  %v9 = call ptr @nil_fn3(ptr %ctx, i64 33, i64 37, ptr %v1)
  call void @nil_root_store(ptr %v10, ptr %v9)
  %v11 = add i64 %v2, 1
  %v12 = call i64 @nil_get(ptr %v1, i64 %v2, i64 47, i64 50)
  call void @nil_root_store(ptr %v8, ptr null)
  %v13 = call i64 @nil_fn2(ptr %ctx, i64 44, i64 51, i64 %v12)
  %v14 = add i64 %v3, %v13
  call void @nil_root_store(ptr %v10, ptr null)
  br label %b1
b3:
  call void @nil_root_store(ptr %v4, ptr null)
  call void @nil_roots_leave(ptr %nil_root_frame)
  ret i64 %v3
}
```

## alias-after

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  br label %b1
b1:
  %v0 = phi ptr [ %p0, %b0 ], [ %v5, %b2 ]
  %v1 = phi i64 [ 0, %b0 ], [ %v6, %b2 ]
  %v2 = phi i64 [ 0, %b0 ], [ %v9, %b2 ]
  %v3 = call i64 @nil_length(ptr %v0)
  %v4 = icmp slt i64 %v1, %v3
  br i1 %v4, label %b2, label %b3
b2:
  %v5 = call ptr @nil_fn3(ptr %ctx, i64 33, i64 37, ptr %v0)
  %v6 = add i64 %v1, 1
  %v7 = call i64 @nil_get(ptr %v0, i64 %v1, i64 47, i64 50)
  %v8 = call i64 @nil_fn2(ptr %ctx, i64 44, i64 51, i64 %v7)
  %v9 = add i64 %v2, %v8
  br label %b1
b3:
  ret i64 %v2
}
```

## slice-after

```llvm
define internal i64 @nil_fn1(ptr %ctx, i64 %call_start, i64 %call_end, ptr %p0) alwaysinline {
b0:
  %nil_root_slots = alloca [5 x ptr], align 8
  store [5 x ptr] zeroinitializer, ptr %nil_root_slots, align 8
  %nil_root_frame = call ptr @nil_roots_enter(ptr %nil_root_slots, i64 5)
  %v0 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 0
  %v5 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 1
  %v6 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 2
  %v8 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 3
  %v11 = getelementptr [5 x ptr], ptr %nil_root_slots, i64 0, i64 4
  call void @nil_root_store(ptr %v0, ptr %p0)
  call void @nil_root_store(ptr %v0, ptr null)
  %v1 = call i64 @nil_length(ptr %p0)
  br label %b1
b1:
  %v2 = phi ptr [ %p0, %b0 ], [ %v2, %b6 ]
  %v3 = phi i64 [ 0, %b0 ], [ %v9, %b6 ]
  %v4 = phi i64 [ 0, %b0 ], [ %v15, %b6 ]
  call void @nil_root_store(ptr %v5, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr %v2)
  call void @nil_root_store(ptr %v6, ptr null)
  %v7 = icmp slt i64 %v3, %v1
  br i1 %v7, label %b2, label %b3
b2:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_root_store(ptr %v8, ptr %v2)
  %v9 = add i64 %v3, 1
  %v10 = call ptr @nil_fn2(ptr %ctx, i64 42, i64 48, ptr %v2, i64 %v3)
  call void @nil_root_store(ptr %v11, ptr %v10)
  %v12 = call i64 @nil_get(ptr %v10, i64 0, i64 48, i64 51)
  call void @nil_root_store(ptr %v11, ptr null)
  %v13 = icmp eq i64 %v12, 10
  br i1 %v13, label %b4, label %b5
b3:
  call void @nil_root_store(ptr %v5, ptr null)
  call void @nil_roots_leave(ptr %nil_root_frame)
  ret i64 %v4
b4:
  br label %b6
b5:
  br label %b6
b6:
  %v14 = phi i64 [ 1, %b4 ], [ 0, %b5 ]
  %v15 = add i64 %v4, %v14
  call void @nil_root_store(ptr %v8, ptr null)
  br label %b1
}
```
