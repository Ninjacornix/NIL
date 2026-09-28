# Compiler architecture

## Planned M1 boundaries

```text
lines-v0 source → parser AST → signature/type checker → HIR validator → evaluator
future frontend ─────────────────────────────────────→ HIR validator
                                                       ↓ later
                                                 MIR → native/WASM
```

- `nil-hir`: semantic types, IDs, operations, structured diagnostics, validation.
  Has no dependency on the parser or source spellings.
- `nil-compiler`: line lexer/parser, syntax AST, resolution and typed lowering,
  reference evaluator. The evaluator consumes only validated HIR.
- `nil`: file/argument handling and rendering. No compiler semantics.

The repository currently contains only the CLI scaffold. Create semantic/compiler
crates when implementing the corresponding M1 tasks; do not add empty crates.
Target no external Rust dependencies for M1. Modules remain small; splitting the evaluator,
parser or diagnostics into crates later requires a real independent consumer.

## HIR invariants (M1)

A program contains functions indexed by `FunctionId`, separate from source labels.
Each function has an explicit parameter/result signature. Parameter values occupy
`0..arity`; each instruction appends one immutable typed value. Operands must refer
to parameters or earlier instructions in that function. All types are currently
`I64`; each operation's output, call arguments, and return must match signatures.
Calls reference existing functions, including forward references and recursion.
Each function has exactly one terminal return. There is no memory, mutable state,
I/O, plugin execution, or control flow. Source spans are optional metadata, not
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
