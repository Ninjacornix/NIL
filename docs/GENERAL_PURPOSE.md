# Application-language development

Target selected by the maintainer: application programs with runtime-sized data,
files and text. C ABI, raw pointers, drivers and hardware access are separate work.
Existing source profiles and their arithmetic/array semantics remain compatible.

## First application core (expr-v5)

- Runtime-sized immutable i64 buffers (`v`) and byte sequences (`s`). Text is UTF-8
  encoded into bytes; byte indexing does not mean Unicode character indexing.
- Quoted UTF-8 literals, explicit length/read/replacement, construction, concatenation,
  slicing, decimal formatting/parsing, and typed file/stdout operations.
- All operations have checked signatures in syntax-independent HIR. Host operations
  are explicit effects: evaluation order and lazy branches remain observable.
- A 64 MiB live-storage budget charges reserved capacity × width plus 40 bytes per allocation.
  This changes the original cumulative quota meaning. Dead native allocations are
  reclaimed, and proved single-use replacements/appends reuse unaliased storage. Copying
  remains the fallback; transient result charges keep reference/native E013 aligned.
- Reference evaluation denies host I/O by default; an explicit host implementation
  grants it. Native application executables use the caller's OS permissions.
- The source profile is experimental. Intrinsic spelling is a provisional generic
  typed-operation call, not a tokenizer-optimized winner or a full plugin ABI.

Acceptance: CLI text parameters, runtime-sized allocation beyond 256 elements,
function/branch/loop transport, immutable aliases, native/reference parity, lazy
I/O, missing-file errors, bounds/byte/allocation failures, and file copy/formatting
applications. Compile and run at O0/O2, retain all prior-profile regression tests.

## Subsequent application work

1. Define recoverable results for I/O failures and injectable filesystem capabilities.
2. Select records, byte/integer widths and floating point using application fixtures.
3. Profile remaining root/runtime/I/O overhead; keep views and ownership syntax separate.
4. Stabilize external calls, modules and object output when integration requires them.
5. Evaluate generated/repair tokens on these larger applications before choosing syntax.

This is a usable application core, not production-completeness certification.
Open decisions: stable ABI, richer effects/capability policy, Unicode operations,
recoverable errors, layout, broader storage optimization, concurrency and deployment targets.

## Validation evidence

197 tests pass in debug and release with strict Clippy. Twenty-one native application
tests pass under ASan/UBSan (leak detection disabled). Three v5 seeds compare 224
programs each against 448 O0/O2 builds each, including alias/trap/append-hostile scenarios.
These remain bounded scenario campaigns, not proofs for arbitrary programs.

Native O2 uses LTO checked C accessors, bulk reads, invariant length hoisting and
conservative root-slot retention. C retains the single access-semantics definition.
Single-use concat has geometric capacity growth; retained aliases still copy.
Capacity accounting intentionally shifts E013 boundaries; the limit stays 64 MiB.
See [append command evidence](validation/APPLICATION_BUILDERS.md) and the
[multi-workload measurements](../benchmarks/reports/2026-10-02/APPLICATION_BUILDERS.md).
Earlier [runtime](validation/APPLICATION_RUNTIME.md),
[application-core](validation/APPLICATION_CORE.md) and [storage](validation/DYNAMIC_STORAGE.md)
evidence is historical. No general native runtime parity, source-token or LLM
repair-efficiency conclusion follows from these application measurements.
