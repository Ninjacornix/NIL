# expr-v3 fuzz testing

The `nil-fuzz` workspace tool provides reproducible mutation/property fuzzing with
an independent typed-tree oracle. It uses only workspace crates and the Rust
standard library; this is not coverage-guided libFuzzer or exhaustive verification.

## Run

```sh
./scripts/fuzz.sh
# 1,000 valid programs, 5,000 mutated/raw inputs, 1,000 HIR mutations,
# 8 native seeds × 3 entries × O0/O2 × bounded/unbounded = 96 builds
./scripts/fuzz.sh --cases 20000 --native 64 --seed 5130572 --out /tmp/nil-fuzz
./scripts/fuzz.sh --cases 20000 --native 0 --seed 100000 --out /tmp/nil-frontend-fuzz
cargo test -p nil-fuzz --locked --offline
```

Clang is needed for native cases. Frontend-only campaigns need only the pinned
Rust toolchain. Cases are independently derived from consecutive u64 seeds; the
first case uses the supplied seed. Select a different output directory for each
concurrent campaign. Counts are capped at one million, with native count no larger
than total count. Successful runs print a JSON summary plus periodic progress lines.

## Properties and capability coverage

- Compile the same source twice: identical acceptance, HIR, diagnostics and IR.
  Check diagnostic codes, nonempty messages and UTF-8 byte-span bounds.
- Mutate source by insertion, deletion, replacement, truncation and Unicode/NUL
  injection; also generate arbitrary bytes. Invalid UTF-8 is rejected at the API
  admission boundary, not passed unsafely as a Rust string.
- Generate typed programs covering all four arithmetic operations, all six signed
  comparisons, bools, lazy branches, forward calls, recursion, mixed loop state,
  parallel updates and nested loops. A test asserts every tree operation is generated.
- Compare compiled-HIR reference execution against an independent tree interpreter.
  Its wrapping oracle uses unsigned arithmetic and widened i128 division. Include
  MIN/MAX, zero, -1 and random i64 inputs. Whole-line whitespace and CRLF changes
  must preserve semantic output and HIR debugging projections.
- Mutate function signatures/results, operation types/operands, callees and nested
  if/loop regions; require deterministic validation and safe handling of accepted HIR.
  Dedicated cases cover expression/HIR nesting and the 1 MiB source boundary.
- Native workers directly exercise all three generated functions, including helper
  and recursive functions skipped by entry branches. Compare O0/O2 and optional
  bounded/unbounded instrumentation with the oracle/reference, including exact
  diagnostic text and exit status. Generated counters/recursion are finite by design.

Mutation-accepted programs may loop or recurse indefinitely. They execute only in
the bounded reference evaluator (256 steps, eight calls). Native unbounded execution
is restricted to the generated terminating family. Each native worker has a 30-second
wall-clock deadline; on Unix, timeout kills its process group, including Clang/native
children. This bounds a hung backend or execution bug, not just normal program fuel.
Frontend checks run in-process under existing compiler source/depth limits.

## Failure artifacts and replay

Before checking an input, the tool saves `current-input.bin` and `current.txt` with
seed/phase. A panic or abort leaves those files. Native failures preserve a
`native-SEED/` directory with source, LLVM IR, mode, arguments, executable when
built, and worker output. Success removes native directories. Generated artifacts
are ignored by Git; promote minimal reproducers into the checked-in corpus/tests.

```sh
./scripts/fuzz.sh --native 0 --replay /tmp/nil-fuzz/current-input.bin --out /tmp/replay
./scripts/fuzz.sh --cases 1 --native 1 --seed FAILED_SEED --out /tmp/replay-native
# Direct native-only replay, using the current generator version:
./target/release/nil-fuzz --worker FAILED_SEED /tmp/replay-worker
```

Source replay checks frontend resilience; semantic/oracle replay uses the seed.
Keep artifact source alongside the seed because future generator versions can change
its program. Direct `--worker` is a debugging entry point without the parent timeout;
use the campaign command for isolated native execution. Every fixed compiler bug
must receive a regression test and, where useful, a minimized corpus input.

## CI and limits

Normal workspace tests include 1,000 seeded property cases, boundaries, generator
capability checks, native smoke tests, timeout handling, replay and failure-artifact
tests. Nightly CI runs 20,000 cases and 64 native seeds and uploads the report and
reproducers even after failure. Timings are informational; property failures fail CI.

The generator deliberately bounds expression depth and loop/recursion counts. It
cannot prove absence of compiler bugs, cover every valid program or measure LLM
repair/TCR. Coverage-guided instrumentation, automated shrinking and larger external
HIR graphs remain useful follow-ups; deterministic byte/source/seed reproducers are
available now.

## Verified campaign

The [2026-09-30 report](../fuzz/reports/2026-09-30.json) records 20,000 generated
programs, 100,000 mutated/raw inputs, 20,000 HIR mutations, 768 native builds and
2,816 native executions without observed crashes or semantic/diagnostic mismatches.
It records toolchain/platform, seed, command and source hashes. Invalid UTF-8 accounts
for 25,799 inputs rejected before the string API; 74,201 mutated/raw inputs reached
the frontend. The debug and release workspace suites each passed 121 tests.

## expr-v4 extension coverage

The campaign CLI still generates expr-v3 scalar programs. Workspace property tests
add 6,000 expr-v4 source mutations and 1,000 typed HIR mutations, using a separate
mutation alphabet so old campaign seeds remain reproducible. Accepted mutations
execute only in the bounded reference evaluator; typed values/diagnostic spans and
deterministic HIR/LLVM are checked. Compiler tests separately compare seeded arrays
of lengths 0..256 against independent sum/reverse oracles. Native integration tests
cover typed calls, array returns, aliasing and traps at O0/O2 in both budget modes.
Run `cargo test -p nil-fuzz --locked --offline` or the full CI script. This does not
claim an arbitrary typed-program generator or coverage-guided array campaign.

## Expr-v5 application differential oracle

Run `./scripts/fuzz-v5.sh --cases 256 --seed 5130572 --out /tmp/nil-v5-fuzz`.
Each generated program is type-checked, evaluated with an in-memory Host and
compiled at O0 and O2. The 64 scenario families vary sizes, integer extremes,
byte payloads, nested sequence expressions, calls, lazy branches and loop state.
They include bounds, quota, byte-range, decimal, filesystem, NUL-path and denied
host failures (E012–E018), including competing invalid inputs. Ten added families stress original reads,
retained caller aliases, post-loop captures, lazy identity arms and returned aliases. Eight additional append families exercise concat aliases, retained callers,
lazy identity arms, post-loop originals, mixed concat/slice/replacement, buffers,
self-appends and formatted reports. Eight further families cover nested scalar
lazy reads/traps, condition regions, ordered effects, allocating siblings, old
reads after replacement, last-use captures followed by allocation, and nested
loops that remain conservative. File fixtures
contain arbitrary binary bytes and are confined to each case's output directory.

Comparisons cover result bytes, diagnostic codes, stdout effects and input/output
file contents, including effects preceding a later failure. Native denial uses
`NIL_DENY_HOST_IO=1`; the memory Host denies the same operations. The reference
uses a one-million-instruction guard so finite quota tests reach allocation
failure; native execution is unbounded with a ten-second process deadline.
Clang builds currently have no separate deadline. Successful cases are deleted;
failures retain source, seed/index, arguments, LLVM IR, binaries and output.
Replay the recorded base seed with `--cases INDEX_PLUS_ONE`; preserve the source
because generator changes can alter a seed's program.

This is a mixed scenario/compositional generator, not arbitrary well-typed program
generation or coverage-guided fuzzing. Summary intrinsic coverage describes
operations appearing in generated source; error counts are observed execution
outcomes. Dedicated scenarios execute each intrinsic. O0/O2 share the LLVM/C
implementation, so agreement with the independent Rust evaluator is essential.
The small corpus under `fuzz/corpus/expr-v5/` seeds frontend/regression tests.
Workspace tests compare one complete scenario cycle; nightly runs four cycles.

Run `./scripts/sanitize.sh` to instrument emitted LLVM code and the C runtime with
AddressSanitizer and UndefinedBehaviorSanitizer at O0/O2. Errors halt execution;
leak checking is explicitly disabled, including on macOS where LeakSanitizer is
unavailable. This command runs native application integration tests, not the entire
fuzz campaign, and does not certify absence of leaks or undefined behavior.
