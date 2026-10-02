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
- A 64 MiB live-storage budget charges payload plus 32 bytes per distinct allocation.
  This changes the original cumulative quota meaning. Dead native allocations are
  reclaimed, and proved single-use replacements reuse unaliased storage. Copying
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

183 tests pass in debug and release with strict Clippy. Thirteen native application
tests pass under ASan/UBSan (leak detection disabled). Two v5 seeds compare 192
programs each against 384 O0/O2 builds each with zero divergences, including ten
added alias/trap-hostile scenarios. A complete 1 MiB file transform succeeds;
its path-dependent verified boundary is 33,554,365 bytes under the unchanged quota.

See [storage command evidence](validation/DYNAMIC_STORAGE.md) and the
[real NIL/C++/Python file benchmark](../benchmarks/reports/2026-10-02/APPLICATION_STORAGE.md).
NIL remains substantially slower than C++ here; feasible storage is not proof of
runtime parity. Source/repair token efficiency remains unmeasured. Earlier
[application-core hardening evidence](validation/APPLICATION_CORE.md) is historical.
