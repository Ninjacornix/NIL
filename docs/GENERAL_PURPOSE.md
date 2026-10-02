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
- An execution-owned allocation arena bounds cumulative dynamic storage to 64 MiB (payload plus 32 bytes per allocation).
  Native allocations are released after the entry result is consumed. Reference
  execution uses immutable shared values and the same cumulative payload budget.
  Replacement copies initially; no claim of general buffer-loop performance.
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
3. Specify buffer views/ownership and prove reuse before optimizing copy-heavy loops.
4. Stabilize external calls, modules and object output when integration requires them.
5. Evaluate generated/repair tokens on these larger applications before choosing syntax.

This is a usable application core, not production-completeness certification.
Open decisions: stable ABI, richer effects/capability policy, Unicode operations,
recoverable errors, layout, reusable storage, concurrency and deployment targets.

## Validation evidence

175 tests pass in debug and release. Formatting, strict Clippy, all-target builds
and documentation tests pass. The v5 differential campaign compared 152 generated
programs against 304 native O0/O2 builds with zero divergences after fixing a byte
construction diagnostic-order bug. It observed E012–E018 and covered all nine
intrinsics, including stdout/file effects and sequence transport.

Run `./scripts/sanitize.sh` for reproducible native application ASan/UBSan checks.
All ten native application tests pass with recovery disabled. Leak detection is
explicitly disabled; this is not a leak-freedom claim. Nightly runs both sanitizer
checks and the seeded differential campaign. See the [command outputs, findings
and limits](validation/APPLICATION_CORE.md). No storage reuse optimization has
started; source/repair token efficiency remains unmeasured.
