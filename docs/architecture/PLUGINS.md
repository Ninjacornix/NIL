# Authoring a version 1 semantic plugin

Version 1 loads **local, validated NIL source at compile time**, then links typed
HIR into the caller's LLVM module. It does not load native objects, dylibs, Python
code or remote packages. This is a versioned semantic interface, **not a stable C
ABI**. There is no sandbox or guarantee that trusted provider code terminates.
See [ADR 029](../adr/029.md) for the boundary and deferred capabilities.

## Worked record plugin

Create a directory containing `plugin.nil-plugin`:

```text
nil-plugin 1
id 1
source bump.nil
effect borrow
types record-buffer
export 0 0
```

`id` is a canonical unsigned integer; zero is reserved for shipped equality.
`source` names a relative UTF-8 regular file, without `..`; manifests and sources
obey the compiler source-size limit. Symlinks are not a sandbox restriction.
`export OP FUNCTION_LABEL` gives operation IDs and provider function labels.
Duplicate IDs/exports, unknown directives/versions and missing exports fail E024.
There may be up to 128 manifests, 128 exports per manifest and 128 functions per
provider. A loaded provider is snapshotted in validated HIR; subsequent file edits
cannot change an already compiled program. No separate native artifact is built.

Write `bump.nil`:

```nil
(Packet):Packet=a{count:a.count+1}
```

The caller supplies the nominal record definition. The loader imports its complete
validated record registry; a provider may use those types but cannot add or redefine
records in version 1. Types resolve statically, including all current scalars,
fixed arrays, sequences, maps and nested records, with existing collection limits.
The compiler owns native aggregate layout; there is no pointer-based user ABI.

| Semantic type | Native representation within the compiled module | Reference value |
|---|---|---|
| i64 / u64 | Typed LLVM i64 bits | I64 / U64 |
| u128 | LLVM i128 | U128 |
| f64 | Strict LLVM double, canonical NaN and signed zero | F64 canonical bit pattern |
| bool | LLVM i1 | Bool |
| Fixed array | LLVM `[N x i64]`, passed/returned by value | Immutable Array |
| Bytes / Buffer | Private arena handle, width 1 / 8 | Immutable Sequence identity |
| Integer / byte / scalar-record maps | Private typed arena map handle | Immutable Map identity |
| Nested record | `%nil.recordN` aggregate in field declaration order | Nominal Record with immutable fields |

LLVM chooses target padding and the actual calling convention. Program-local
record IDs are resolved from names and complete field schemas; packed scalar-record
map values retain ADR 028's layout. Handles are never exported as NIL integers.
Unknown types/version capabilities are rejected, not reinterpreted as pointers.

ADR 030 adds optional `types record-buffer` to version 1. It is required when
the imported complete registry, provider signature or body uses `v[Record]`;
absence, duplicate or unknown capabilities fail E024. Old-type bundles remain
compatible without it. Record buffers pass as private arena handles and immutable
reference values, with transitive child roots and the same live-capacity quota.
This advertises support for a core type, never permission to define one.

A client, `client.nil`:

```nil
record Packet(count:i,text:s)
=!plugin(1,0,Packet(41,"kept")).count
```

Build the compiler, validate the bundle in its client's type context, and build/run:

```sh
cargo build --release -p nil --locked --offline
./target/release/nil --profile expr-v5 --plugin plugins/example/plugin.nil-plugin check plugins/example/client.nil
./target/release/nil --profile expr-v5 --plugin plugins/example/plugin.nil-plugin hir plugins/example/client.nil
./target/release/nil --profile expr-v5 --plugin plugins/example/plugin.nil-plugin build plugins/example/client.nil -O2 -o /tmp/nil-plugin-client
/tmp/nil-plugin-client
# 42
```

The checked-in `plugins/example/` is this complete bundle. All commands accept
repeatable `--plugin MANIFEST` after `--profile` and before instrumentation/command.
Loading is explicit; a source call cannot pick a filesystem path. Rust integrations
use `nil_compiler::compile_with_plugins(source, SourceProfile::ExprV5, paths)`.
The resulting `CompiledProgram.hir` runs through the existing evaluator and native
APIs; no special plugin runner is needed after linking.

## Contracts that are enforced

`borrow` permits **no dynamic-arena allocation and no host effect** anywhere in the
provider's reachable validated HIR. The declaration does not grant permission:
interprocedural analysis independently proves it. Unknown/allocating/host effect
contracts fail E024. Recursive and nested plugin providers are rejected in this
prototype; ordinary acyclic internal function calls and loops work. Checked scalar
operations can trap; borrow does not mean nontrapping. The validator checks every
provider function, including unexported ones, conservatively.

Providers receive immutable values owned by the caller's arena. No raw address is
representable. A returned sequence/map/record leaf can only retain an existing
allocation identity; caller/result roots and the ordinary live-capacity quota apply.
Static product updates can return shared sequence fields without copying them.
Allocating operations such as sequence replacement/concat/map update, including
those inside unselected arms, invalidate the borrow proof. Host I/O cannot hide
behind a pure declaration. Narrow dead-field root transfer follows ADR 031; other aliases remain rooted.

Calls evaluate arguments once, left-to-right, inside the selected lazy arm. Core
E006/E007/E023 apply to typed source errors; E024 applies to linking/contracts.
Provider operations retain ordinary failure codes and executed trap order. Provider
source diagnostics refer to its imported-record prefix plus body offsets; no stable
cross-file diagnostic format is promised yet.

The caller spends **one instruction** for the exported call. Reference provider
execution uses `u64::MAX` internal steps and depth 256; acyclic providers have at
most 128 functions. Native provider bodies do not consume caller fuel/depth. This
matches legacy intrinsic accounting and does not add hidden loop costs to `equal`.
The shipped equality loop terminates within the allowed sequence length, well below
that internal step limit. An arbitrary trusted loop can still run indefinitely;
version 1 is not a CPU sandbox. Native and reference interpret/compile the same
validated body, but goldens and adversarial cases are still needed to validate the
algorithm itself. Differential agreement alone is not correctness.

## Migration and what remains open

`!plugin(0,0,lhs,rhs)` selects the shipped equality operation, with the same
typed overloads. It requires no external manifest. `!equal(s,s)->b` and
`!equal(v,v)->b` are compatibility spellings and now execute the shipped sequence provider in
`plugins/sequence/`. Its portable HIR recipe is the actual implementation;
`equal.nil` documents the equivalent source. The old `Intrinsic::Equal` Rust enum
variant survives as an **input compatibility adapter** and is normalized during
HIR validation; no equality evaluator or C runtime implementation remains in core.
This migrates one algorithm without breaking existing source/API fixtures.

Version 1 does not support allocating/effectful providers, a stable foreign ABI,
runtime dispatch or arbitrary native loading. Those need explicit owned-result,
capacity reservation, root transfer and host capability protocols, plus a trust
review. New core types require version/capability review instead of silent native
layout compatibility. This prototype is useful for borrowed algorithms and static
product helpers; it does not yet move output builders/JSON libraries out of core.


## General bulk-scan lowering

ADR 031 proves a complete equality-scan idiom from validated provider HIR and can
lower it to a private optimized bulk comparison. It neither reads IDs/names nor
trusts a manifest assertion. Any third-party provider using the proved length guard,
zero initial index, unit stride, short-circuit equality accumulator and exact exit
result qualifies; altered or unproved bodies compile normally. Dead instructions
are checked too, including reads that could trap before the guard. This is a
specific compiler optimization over visible code, not a promise that arbitrary
plugin algorithms receive bulk lowering. Borrow/effect summaries still come from
the body, and the reference evaluator interprets it unchanged. No types/intrinsics
were migrated or added by this optimization.

Known matcher limitation: commutative equality is not normalized. A provider
using `#b==#a` and `b[c]==a[c]` instead of `#a==#b` and `a[c]==b[c]` keeps the
ordinary checked loop, without a diagnostic. The overseer's independent ID-8
provider measured 16.57 ms on the 16 MiB control, versus 7.57 ms for the matched
ID-7 provider. These are externally reported measurements, not a new local run.
The fallback preserves semantics; canonical equivalent operand ordering can
therefore change performance. This limitation is recorded, not fixed here.

## General modules through this loader

[ADR 032](../adr/032.md) generalizes the same bounded reader, manifests, export
resolution, source snapshots and linker with `nil-module 1`. It permits allocation,
host effects, local recursion and transitive `import RELATIVE_MANIFEST` declarations.
The loader relocates local function labels and resolves exported `!plugin(ID,OP,
args...)` references to **ordinary HIR calls** before whole-program validation.
Modules therefore use the existing borrowing/effect summaries and caller-owned
arena/root tracking, including conservative handling when callees may allocate.
They use ordinary caller fuel/depth; restricted plugins retain their existing
atomic accounting and borrow-only proof. No second runtime or foreign ABI exists.

Every manifest has a nonzero global ID; only direct imports' explicit exports are
visible. Shared canonical paths load once, conflicting IDs and import cycles fail
E024. Root CLI entry selection cannot address relocated private functions. The
root remains the owner of the nominal record registry. Modules cannot define new
types in this prototype. Source spans remain local offsets rather than file IDs.

`--module` and `--plugin` feed the same loader and accept either supported manifest
kind. No source import directive or convenience intrinsic is added. Existing plugin
capabilities/restrictions still apply to `nil-plugin 1`, even when a module imports
one. A module does not implicitly inherit transitive exports. See the full limits
and failure rules in [EXPR_V5](../language/EXPR_V5.md#local-source-modules-adr-032).

A worked bundle is in `examples/expr-v5/modules/`: two file-processing entry points
share newline counting, integer reduction and formatting code. Run:

```sh
printf 'a\nb\n' > /tmp/nil-module-input
./target/release/nil --profile expr-v5 --module examples/expr-v5/modules/reports.nil-module run examples/expr-v5/modules/count_newlines.nil 0 /tmp/nil-module-input /tmp/nil-module-output
cat /tmp/nil-module-output
# CLI result: 2 (bytes written); file contents: 2 followed by a newline
```

Trusted local source only; this relaxation adds no sandbox, native artifact,
registry, remote fetching or permission to leak arena handles into NIL integers.

Environment, entropy and directory acquisition (ADR 033) are core host effects.
Ordinary source modules may use them with the caller's Host, quota and root arena.
Borrow-only providers cannot acquire these privileges; the whole-body borrowing
check rejects them even if their result is scalar. Algorithms built on these
capabilities remain candidates for compiler-visible library/module providers.
