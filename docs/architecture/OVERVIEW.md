# Compiler architecture

## Implemented M1/M2 boundaries

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

## HIR invariants (M1/M2)

A program contains functions indexed by `FunctionId`, separate from source labels.
Each function has an explicit parameter/result signature. Parameter values occupy
`0..arity`; each instruction appends one immutable typed value. Operands must refer
to parameters or earlier instructions in that function. Types are `I64` and `Bool`; each operation's output, call arguments, and return must match signatures.
Calls reference existing functions, including forward references and recursion.
Each function has exactly one terminal return. There is no memory, mutable state,
I/O, plugin execution, or external effects. Structured branches and typed state-tuple loops use regions
with lexical operand scopes and explicit yields (see [M2 invariants](../language/CONTROL_FLOW.md)). Source spans are optional metadata, not
semantic identity. The validator also handles externally constructed HIR.

The AST holds unresolved function labels and unvalidated operands; lowering resolves
labels in source order and propagates operation types deterministically. A private
validated-program wrapper prevents execution of unchecked HIR. Future frontends may
construct HIR directly and call the same validator; they need not construct this AST.

## MIR proposal (not implemented)

M4 will specify typed SSA basic blocks with block arguments, terminators, dominance,
predecessor arity/type checks, and deterministic numbering. Structured HIR regions
may lower to that form after M2 experience. Effects and memory operations must be
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
