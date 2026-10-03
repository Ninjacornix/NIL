# Compiler architecture

## Implemented compiler boundaries

```text
source profiles → AST → typed/validated HIR → LLVM SSA CFG → Clang → native code
future frontend ────────────────────────┘
                            └→ reference evaluator (tests/benchmarks)
future portable MIR/plugin/WASM boundaries remain unimplemented
```

- `nil-hir`: semantic types, IDs, operations, structured diagnostics, validation.
  Has no dependency on the parser or source spellings.
- `nil-compiler`: profile lexer/parser, syntax AST, resolution and typed lowering,
  reference evaluator. The evaluator consumes only validated HIR.
- `nil-llvm`: syntax-independent HIR-to-LLVM SSA lowering and host compile/link.
- `nil`: file/argument handling, native run/build and diagnostic rendering.

The four crates have no external Rust dependencies. Parser, lowering and evaluator
are separate modules inside nil-compiler. Splitting them into crates later requires
an independent consumer. The compiler wrapper keeps source labels outside HIR;
HIR dump output omits spans and source labels and is only a debugging projection.

## HIR invariants

A program contains functions indexed by `FunctionId`, separate from source labels.
Each function has an explicit parameter/result signature. Parameter values occupy
`0..arity`; each instruction appends one immutable typed value. Operands must refer
to parameters or earlier instructions in that function. Types are `I64`, `Bool`, and fixed `Array(N)` of i64; each operation's output, call arguments, and return must match signatures.
Calls reference existing functions, including forward references and recursion.
Each function has exactly one terminal return. There is no exposed memory, mutable state,
I/O, plugin execution, or external effects. Structured branches and typed state-tuple loops use regions
with lexical operand scopes and explicit yields (see [M2 invariants](../language/CONTROL_FLOW.md)). Source spans are optional metadata, not
semantic identity. The validator also handles externally constructed HIR.

The AST holds unresolved function labels and unvalidated operands; lowering resolves
labels in source order and propagates operation types deterministically. A private
validated-program wrapper prevents execution of unchecked HIR. Future frontends may
construct HIR directly and call the same validator; they need not construct this AST.

## MIR proposal (not implemented)

A separate portable MIR is conditional on an independent consumer or concrete
compiler need. If selected, M4 will specify typed SSA blocks, terminators, dominance,
predecessor arity/type checks and deterministic numbering. Existing structured
HIR already lowers to backend-specific LLVM CFGs. Effects and memory operations must be
explicit before reordering/optimization is allowed. No assumption that every
semantically equivalent algorithm has an identical canonical form.

## Plugin extension seam (design only)

A future generic call carries plugin identity, operation identity and value operands.
Resolve a versioned registry signature before constructing typed HIR. Metadata needs
stable identity, debug name, input/output types, effects/capabilities, lowering and
runtime requirements. Local dense aliases may map to stable identities. Plugins
cannot inject parser productions. C ABI/opaque handles and intrinsic lowering are
candidates, not implemented commitments. Start M8 with one tiny test plugin.

## Native execution update

`nil run` now compiles and runs LLVM-native code by default. Internal typed operands,
basic blocks and phi nodes implement M2 region lowering inside nil-llvm; this is not
a finalized portable MIR serialization. The separate reference evaluator remains
the differential oracle. [Native invariants](../language/NATIVE_LLVM.md) and
[ADR 012](../adr/012.md) document ABI, guards and toolchain boundaries.

## Arithmetic and execution policy

HIR Program carries Checked/Wrapping integer semantics, independent of syntax.
All functions in one module share the mode. expr-v3 lowers the unchanged v2 AST
with Wrapping; earlier profiles lower with Checked. LLVM uses plain modular
operations for Wrapping and overflow intrinsics for Checked. Both retain defined
division behavior and lazy regions. Bounded accounting is a separate backend option;
v3 omits it by default, while the reference evaluator remains a bounded oracle.
See [ADR 013](../adr/013.md) and [v3 semantics](../language/EXPR_V3.md).

## M3 typed arrays

Arrays are immutable values, with length 0..256 in the static type. Construction
and repeat require i64 elements; indexing/replacement require i64 indices and
mandatory bounds traps. Replacement yields a new equal-length value; calls,
regions and loop yields must match exact types. Function input payload is capped
at 4096 flat i64 slots. Array length is known statically. No pointers escape.

The evaluator shares immutable array storage; replacement copies privately. LLVM
uses aggregate SSA operands/phi joins and guarded storage for dynamic indexing.
All temporary allocations occur in the function entry block, not inside loops.
Immutable parameters are snapshotted once; identity-carried loop state can reuse
that read-only snapshot. Replacement never writes through cached storage.
Def-use analysis proves single-use replacement chains, including conditional
updates, and lowers qualifying loop state to separate private buffers. Collect
writes during body evaluation and commit after body yield; inactive branch writes
never touch storage. Unsupported/escaping chains retain aggregate lowering. See
[ADR 015](../adr/015.md) for alias, scope and instruction-order invariants.
Typed native entries use a generated flat-slot bridge and JSON array/bool output;
legacy all-i64 entries keep their original ABI. See [expr-v4](../language/EXPR_V4.md).

## Application sequences and host effects

V5 HIR adds Bytes/Buffer types, byte literals and typed intrinsic calls. Both types
use immutable values through functions/regions. LLVM lowers them to opaque pointers
owned by an execution arena; source has no pointer access. Bounds, byte ranges and
allocation quotas are mandatory. The generated application driver consumes results
before releasing the arena. Fixed-array def-use storage optimization remains separate.
Reference execution uses shared immutable sequences with equivalent accounting.
Read/write/out cross an explicit Host boundary in reference execution; opaque native
runtime calls preserve effects and ordering. See [ADR 017](../adr/017.md).

## Structured iteration lowering

Expr-v5 each is a typed syntax node, not a separate evaluator/backend engine.
A source-binding table maps its index/key, element/value and user states into a
HIR Loop's hidden snapshot/index plus user state. Synthetic prefix instructions
bind checked elements or owned map entries; the finish region exposes only user
state. Lazy regions inherit the binding table; nested loops/each create their own.
This keeps source IDs distinct from HIR IDs and preserves evaluation order after
an each expression. HIR validation and existing root/last-use proofs remain the
execution boundary. Sort is a typed HIR intrinsic with deterministic evaluator/C
runtime implementations and guarded unique-storage reuse. [ADR 025](../adr/025.md).
