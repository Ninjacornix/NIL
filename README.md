# NIL — Neural Instruction Language

NIL is an experimental programming language for LLM-generated programs. It aims to
reduce the total tokens needed to produce a correct program, including failed
attempts and repairs. Compact source is useful only when the model can generate it
reliably.

The compiler is written in Rust. It checks types, lowers programs to a semantic IR,
and emits LLVM IR for Clang to compile into native executables. Different source
representations share the same compiler core.

## A small program

```text
f0()=f1(20,22)
f1(a,b)=a+b
```

This program returns `42`. Function 0 is the default entry point; function 1 takes
two signed 64-bit integers and returns their sum.

The default syntax is [expr-v0](docs/language/EXPR_V0.md). The experimental
[expr-v3](docs/language/EXPR_V3.md) profile expresses the same program as:

```text
=b(20,22)
2=a+b
```

In expr-v3, functions are identified by declaration order (`a`, `b`, `c`, …), and
parameters by position. A leading number declares the parameter count. Calls and
parameter references are distinguished by parentheses.

NIL supports integer arithmetic, function calls and recursion, comparisons, lazy
conditional expressions, and loops with explicit state. Earlier profiles use `i64` function
signatures. The opt-in
[expr-v4 profile](docs/language/EXPR_V4.md) adds typed bool/array signatures and
immutable fixed-size integer arrays with checked indexing and replacement.
The opt-in [expr-v5 profile](docs/language/EXPR_V5.md) adds application storage,
`u64`, `u128` and deterministic binary64 scalars with explicit conversions. Exposed
memory operations remain unimplemented; local validated semantic plugins are prototyped.
V5 also exposes explicit environment lookup, injectable randomness and deterministic
directory snapshots; reference execution denies them unless a Host is supplied.
See [ADR 033](docs/adr/033.md) for the core/plugin backlog and entropy contract.

## Source standard library (expr-v5)

`!map(&c,a)`, `!filter(&c,a)` and `!fold(&c,a,0)` call compiler-shipped NIL
source automatically, with no imports or manifest. They support Bytes and i64
Buffer; callbacks are statically specialized. Same-name calls cost the same
cl100k/o200k tokens as intrinsic calls. `!7.0(a)` also saves two tokens versus
`!plugin(7,0,a)` for explicitly loaded module exports.

**Find, has and parse remain core:** all three source migration candidates failed
the speed or semantics gate. Std map/filter/fold match identical handwritten loops
at O2, including a cross-module scan that remains vectorized. Guarded callee
updates and singleton builders improve existing programs without new syntax.
An independent audit found no unsound deferred-root case. Proved stable updates
recover 16 MiB map/transform to **36.19/35.80 ms**, from **48.51/44.62 ms**;
unbounded std `!push` now matches concat (**78.73/78.94 ms** on the conditional
builder). Scan remains vectorized at **8.67 ms**; fixed-bound 1 MiB append is
**6.02 ms** versus C++'s **5.20 ms**.
**C++ parity is still incomplete:** std filter is 81.98 versus 29.82 ms,
transform 35.80 versus 13.06 ms, and comparator insertion sort 49.35 versus
4.90 ms. All five 250k-entry map controls with 64-byte keys hit the existing
quota during construction; iteration copies keys and hashes them again. Source
comparator merge sort remains a prototype; core `!sort` stays. These are
one-machine whole-process measurements, 25 repetitions, not universal ratios.
An initial 4–9% source-sort slowdown was recovered with a preregistered macOS
allocation annotation: 1M elements now take 1,438 versus baseline 1,458 ms.
Portable allocation and both sanitizer paths remain tested; quota rules are unchanged.
35 intrinsic variants and 34 executable core implementations are unchanged.
Corpus sources/token counts and earlier negative generation findings are unchanged.

[Measurements, proof boundaries and gate receipts](benchmarks/reports/2026-10-06/ROUND18.md)
include 466 debug/release tests, 199 sanitizer tests, three fully covered 475-family
fuzz campaigns and 2,815 corpus checks (original group exactly 610).

## Build and run

Install Rust through rustup; the repository pins the development toolchain in
`rust-toolchain.toml`. Compiling NIL programs also requires Clang 15 or newer,
system C headers, and a linker. On macOS, install Apple's command-line developer
tools. Expr-v5 O2 uses link-time optimization; on Linux also install LLD
(`apt-get install clang-18 lld`). Rust dependencies are confined to this workspace.

```sh
cargo build --release -p nil --locked --offline
./target/release/nil run examples/add.nil
# 42
./target/release/nil build examples/add.nil -o /tmp/nil-add
/tmp/nil-add
# 42
```

`run` compiles and executes a program. Use `build` to keep the executable for
repeated runs. `check`, `hir`, and `llvm` validate source or print intermediate
representations:

```sh
./target/release/nil check examples/add.nil
./target/release/nil hir examples/add.nil
./target/release/nil llvm examples/add.nil
./target/release/nil --profile expr-v3 run examples/expr-v3/weighted.nil
# 33
```

Earlier profiles check integer overflow and apply execution limits. Expr-v3/v4 use
wrapping arithmetic and runs without those limits by default; division by zero
still traps. Add `--bounded` after the profile to enable execution limits.

macOS release packages support Apple Silicon and Intel. Downloaded compilers do
not require Rust, but `build` and `run` still require Clang. See
[installation instructions](docs/RELEASES.md).

Try a typed array program with expr-v4:

```sh
./target/release/nil --profile expr-v4 run examples/expr-v4/reverse.nil 0 1 2 3 4 5 6 7 8
# [8,7,6,5,4,3,2,1]
```

`0` selects the entry function; the following eight arguments form its array.
Array length is statically checked, and out-of-range access traps.

Try application data with the opt-in expr-v5 profile:

```sh
./target/release/nil --profile expr-v5 run examples/expr-v5/sum.nil 0 '[1,2,3,4]'
# 10
./target/release/nil --profile expr-v5 run examples/expr-v5/greet.nil 0 World
# Hello, World
./target/release/nil --profile expr-v5 run examples/expr-v5/copy.nil 0 input.bin output.bin
```

V5 adds runtime-sized integer buffers, byte/text values and explicit file operations.
Its 64 MiB budget counts live reserved capacity and transient results. Proven
unaliased replacements and appends reuse storage; geometric growth makes incremental
building amortized linear for byte/i64 and scalar-record storage. Dynamic-child
roots are retained for proven single-use concat state. Updates through live parent
rows and genuine aliases still copy. Aliases retain immutable-value behavior.
O2 inlines checked C accessors through LTO and uses bulk file reads.
Nonallocating scalar conditionals retain loop roots outside the backedge. See the
[before/after file-transform measurement](benchmarks/reports/2026-10-02/APPLICATION_RUNTIME.md);
see also [append and broader application measurements](benchmarks/reports/2026-10-02/APPLICATION_BUILDERS.md).
The [lazy-region before/after study](benchmarks/reports/2026-10-03/LAZY_REGIONS.md)
includes the branchless scan control and conservative call boundary.
These results establish specific workloads, not general C++ performance parity.
See [application examples](examples/expr-v5/README.md) and [the memory/effect contract](docs/language/EXPR_V5.md).
**Standing capable-model evaluation remains unmeasured.** The Claude Pro transport
now uses private OAuth-token-file authentication, isolated sibling temp state and
rate-limit pause/resume. The earlier EPERM was a harness defect. Offline tests use
fake tokens and mocked CLI responses; real preflight and inference are left to the
maintainer. The 24-task Python/spec-only/few-shot study has a USD50 notional cap
and corrected thinking-inclusive output limits. [Protocol and commands](benchmarks/generation/standing/README.md);
[offline validation](docs/validation/V5_SUBSCRIPTION_EVAL.md).

It remains experimental. In the [frozen application generation study](benchmarks/reports/2026-10-03/V5_GENERATION.md),
expr-v5 spent more input-plus-output tokens and solved **0/24** trials; Python
solved **11/24**. This was an unfavourable result for Gemma 3 4B and Qwen 2.5 7B
under the fixed prompts and repair budget. Zero v5 solves makes its TCR undefined;
the small study does not establish a universal language ranking.

**The fixed few-shot study changed failure modes: NIL solved 0/24
control and 0/24 with four verified examples; rerun Python solved
11/24.** Parsing passed 0/72 control attempts versus
53/72 few-shot; 18 few-shot attempts typechecked
and 18 compiled. Static-stage success is not task correctness: the report separates
wrong-task example copies, type errors and semantic/execution failures. Input/output
and repair totals are charged in full; zero-solve TCR is undefined. This is four
tasks, three seeds, two small quantized models and one rule, not evidence that
fine-tuning works or that NIL saves total model tokens.
All 18 compiled treatment candidates exactly copied the newline-count example
for the wrong tasks. Failure-inclusive input/output totals were **72,409 / 13,142**
control versus **106,209 / 10,892** few-shot: **36.9% more total tokens**, with
no correct NIL program. The narrower imitation effect does not establish adaptation
as a fix for correctness.
[Few-shot results, token totals and limits](benchmarks/reports/2026-10-05/V5_FEWSHOT.md).

The [expanded application corpus](benchmarks/corpora/application-v5/README.md)
retains **79 tasks: 25 self-authored and 54 externally derived**, with 68 verified
NIL programs. Four new algorithm references demonstrate indexed lists/trees,
nested graph adjacency and interval merging; the frozen external BST is newly
supported. All old sources/counts remain unchanged. **NIL uses 26.9–36.4% more
source tokens than Python externally, and 19.1–27.1% more combined.** It uses
35.4–39.1% fewer than C++ externally and 41.8–45.0% fewer combined. These ranges
cover Gemma, Qwen, cl100k and o200k. The four new algorithms alone cost 66.6–67.6%
more than Python and also lose to C++ on three of four tokenizers.

**Eleven external tasks lack verified NIL solutions**. Environment lookup now
passes the unchanged corpus oracles. The three new host effects did not deliver
the target three verified tasks: Diffie–Hellman needs a concrete property oracle,
and grep still needs fixture mounting and a JSON/flag adapter.
The original 24 retain their 3.7–8.2% source advantage over Python. The widening
is cohort movement, not increased cost on old tasks; source density does not
establish model efficiency or overturn the failed generation study above.
The expanded original 25 (including environment) now use 6.2–10.6% fewer Python
source tokens. Old sources/counts are unchanged.
[Current host effects, density and validation](benchmarks/reports/2026-10-05/V5_HOST_EFFECTS.md).

V5 supports insertion-ordered byte-keyed maps with integer, byte-string or scalar
record values. Immutable records have named fields, functional updates and nesting.
[ADR 030](docs/adr/030.md) adds `v[Record]` and `!buffer[Record](length,fill)`;
existing indexing, replacement, concat, slice and snapshot each work on them.
Integer links represent nodes, while wrapper records represent nested collections.
No recursive types, addresses or cyclic ownership graph are introduced. Sequence-bearing
record map values and nested map values remain deferred. Record sorting/comparators
and general JSON/domain adapters remain library work; the interval reference uses
insertion sort. Proven dead-field append and single-use fresh/shared record-buffer
builders now scale approximately linearly. The [performance-debt study](benchmarks/reports/2026-10-05/V5_PERFORMANCE_DEBTS.md)
measures 160k field appends at **7.57 ms**, versus 396.65 ms with the starting
compiler. Bulk equality is restored through provider-body recognition, available
to third-party providers: **16.97 → 7.22 ms** on 16 MiB at O2.
The comparison-dependent field slowdown was isolated to root inlining/code layout
in the old forced-copy workload; removing those copies resolves the direct case.

Current 25-repeat controls are **22.84 ms append (1 MiB), 7.56 ms scan and
36.19 ms transform (16 MiB)**. Fields inside a still-live record-buffer
row remain quadratic, as do genuinely aliased builders; their scaling curves are
published. Size-changing keyed payload repacking and unproved allocating nested
regions remain conservative. No bounds, quota, alias or effect checks were weakened.
A validated type scan keeps the established eager flat-arena path; child-aware
programs use bounded deferred collection. These are measured workload results,
not general C++ parity. The zero-slot layout regression remains covered by O0/O2,
sanitizers and twenty degenerate-layout fuzz families.

[Token attribution](benchmarks/reports/2026-10-03/V5_ATTRIBUTION_PLAN.md) accounts
for the external cl100k surplus of 650 tokens. Numeric literals and explicit
algorithms dominate; Grade School, Connect and ETL contribute 523 of that net gap.
Whole-source forecasts save only 32 tokens from signature references and nine
from omitted unchanged loop state. These proposals remain unimplemented; no
surface compression was justified by that evidence. See [ADR 024](docs/adr/024.md).

[ADR 025](docs/adr/025.md) preregistered sorting and structured iteration instead.
Verified rewrites save **398 of the 650 cl100k surplus tokens**, matching the
forecast. Eleven external programs improve; 21 do not. The Python gap above
remains. `sort` preserves immutable aliases and deterministic key/value ordering;
`each` lowers to existing typed HIR loops with snapshot and parallel-state semantics.

[ADRs 026–027](docs/adr/026.md) separate fundamental core contracts from future
compiler-visible extensions. Binary64 uses strict, unfused arithmetic, canonical
NaNs and bit-exact reference/native checks. `u64` and `u128` make Grains and
Armstrong Numbers expressible; floats enable Darts and the selected Complex Numbers
real-part property. No other widths, numeric collections or plugin ABI were added in that round.
[ADR 028](docs/adr/028.md) adds bounded inline records with rooted dynamic fields.
Construction, projection and update preserve aliases; the intrinsic count remains
32. Record-field sequence append remains conservative and can copy per iteration.

[Version 1 semantic plugins](docs/architecture/PLUGINS.md) load local NIL providers
and link validated HIR into native code. Borrowing and effects are derived from
bodies, not trusted annotations. Equality now lives in the shipped sequence
provider; a worked plugin updates records without allocating. Allocating/effectful
providers and arbitrary native libraries remain unsupported. This is a semantic
linking prototype, not a stable C ABI or a sandbox. At migration the intrinsic enum kept its
compatibility adapter: 31 executable core implementations plus migrated equality;
the three host contracts bring the current enum count to 35.
[Measured boundary costs](benchmarks/reports/2026-10-04/V5_PLUGINS.md) retain the
material bulk-comparison regression: 16 MiB equality rose from 7.23 to 16.86 ms
at O2, while repeated calls preserved borrowing/reuse proofs. An optimized bulk
lowering is needed before treating this migration as production-ready.

Expr-v5 also supports [local source modules](docs/adr/032.md) through the same
loader. `nil-module 1` permits allocation, host effects, local recursion and
transitive imports; `nil-plugin 1` retains its proved borrowing contract. Imported
exports use `!plugin(ID,OP,args...)` and become ordinary HIR calls before validation.
Load a manifest with `--module FILE`; root function 0 remains the entry. Types are
still declared in the root, and import cycles are rejected. The
[shared reporting examples](examples/expr-v5/modules/README.md) preserve existing
corpus outputs. Small splits cost more source tokens: the two-example bundle adds
33–39 tokens including its manifest. Single-file sources need no changes.

The external tasks are the first 50 alphabetical Exercism specifications at a
pinned revision, adapted to their first declared property. This is a nonrandom,
partial-API sample with locally authored solutions and finite upstream oracles.
Source density does not establish model efficiency or reverse the unfavourable
generation result above. The licensed corpus exports 61 adaptation seeds; it is
still far below a usable fine-tuning set. No training or inference occurred.

## Compiler and examples

- [`crates/nil-compiler`](crates/nil-compiler): parsing, type checking, lowering, and the reference evaluator.
- [`crates/nil-hir`](crates/nil-hir): semantic operations, types, validation, and diagnostics.
- [`crates/nil-llvm`](crates/nil-llvm): LLVM emission and native compilation.
- [`cli/nil`](cli/nil): the compiler command-line interface.
- [`examples`](examples): runnable arithmetic, control-flow and typed-array programs.

The [architecture notes](docs/architecture/OVERVIEW.md) describe the compiler
boundaries. The reference evaluator provides an independent execution path for
checking native results. Source profiles remain experimental; their grammar and
semantics are documented in [`docs/language`](docs/language).

## Tests and measurements

```sh
./scripts/ci.sh          # formatting, linting, build, and tests
./scripts/ci.sh release  # release build and tests
./scripts/fuzz.sh        # seeded mutation and differential testing
./scripts/fuzz-v5.sh     # application reference/native and host-effect comparisons
./scripts/sanitize.sh    # native application ASan/UBSan checks
```

Tests cover invalid source, typing, IR invariants, diagnostics, and execution.
[Fuzz testing](docs/FUZZING.md) compares generated programs against the reference
evaluator and native code at different optimization levels.

[Benchmarks](https://github.com/Ninjacornix/NIL-benchmarks) compare source tokens and runtime with
other languages. The [expr-v3 results](benchmarks/reports/2026-09-30/EXPR_V3.md)
cover the integer core. The [expr-v4 results](benchmarks/reports/2026-09-30/EXPR_V4.md)
record the initial typed-array experiment. Private loop storage now removes
copying for proven replacement chains; the [storage benchmark](benchmarks/reports/2026-09-30/EXPR_V4_STORAGE.md) compares this against
fresh-local C++ and Python. These source/runtime measurements do not establish
model efficiency. The [v5 generation experiment](benchmarks/reports/2026-10-03/V5_GENERATION.md)
counts actual input, output and failed repair tokens. It found no token-efficiency
benefit on its four application tasks; full per-task results and limitations are
reported rather than inferred from source length.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development commands and contribution
guidelines.

## License

NIL is available under the [MIT License](LICENSE).

Benchmark tools and corpora are an optional pinned submodule. Initialize them with
`git submodule update --init benchmarks`; compiler builds and tests do not need it.

Static higher-order functions in expr-v5 use `[i:i]` parameters, `&c` references
and `^a(value)` calls. They specialize to direct calls before HIR, including across
modules; intrinsics remain 35. `nil --profile expr-v5 run examples/expr-v5/higher_order.nil` prints 21. No closures or runtime-selected function
values are supported. Accumulate is newly oracle-verified, but its NIL reference
costs 2.87–3.10 times Python's source tokens; this addition explains the widened
density headline above. Map controls are at direct-call parity, while the HOF
insertion-sort control is much slower than core sort. Validation: 422 tests, 169
sanitizer tests, 2,815 corpus checks (original 610 unchanged).
[Higher-order functions: costs, evidence and limitations](benchmarks/reports/2026-10-06/V5_HIGHER_ORDER.md).
