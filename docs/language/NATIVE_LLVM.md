# LLVM native compilation

LLVM is the default execution backend. All current expression profiles and
lines-v0 compile through the same validated HIR. Source syntax is unchanged.

```text
source → AST → typed/validated HIR → LLVM SSA blocks/phi nodes
       → Clang optimization/code generation → linker → native executable
```

The interpreter remains in the compiler library for semantic tests and benchmarks;
`nil run` does not use it or fall back to it.

## Requirements and commands

[expr-v3](EXPR_V3.md) adds an opt-in wrapping mode without default resource counters.
The checked semantics described below apply to earlier profiles.

Native compilation requires Clang 15+ (opaque-pointer LLVM), system C headers and
a linker. Supported build hosts are 64-bit macOS and Linux; build targets the host.
On macOS use Apple's command-line developer tools. Ubuntu CI installs clang-18.
Set `NIL_CLANG=/path/to/clang` to choose another executable, not shell flags.
Rust is required to build the NIL tool, but not to use its downloaded executable
or run an already-built NIL program. Clang is required for `run` and `build`;
`check`, `hir`, `llvm`, and existing compiled executables do not require it.

```sh
cargo build --release -p nil --locked --offline
./target/release/nil run examples/fibonacci.nil 1 92
# 7540113804746346429
./target/release/nil build examples/fibonacci.nil --entry 1 -o /tmp/nil-fibonacci
/tmp/nil-fibonacci 92
# 7540113804746346429
./target/release/nil --profile expr-v2 build benchmarks/paired/control-samples/factorial.v2.nil -o /tmp/nil-factorial
/tmp/nil-factorial 10
# 3628800
```

`run FILE [FUNCTION_ID [ARGUMENT...]]` compiles an O2 executable privately, executes
it, forwards its result/diagnostic and removes build files. It recompiles on each
invocation: use `build` for repeated execution. `build` defaults to source function
0 and O2; `--entry ID` selects another function and `-O0` disables optimizations.
`-O2` is also explicit. Pass only function arguments to the generated executable;
its entry function was selected at build time. A failed build preserves any existing
output; the CLI rejects overwriting the source. Output parent directories must exist.

```sh
./target/release/nil llvm examples/max.nil > /tmp/max.ll
```

This emits a deterministic LLVM function module and context guards, not a complete
standalone main. `build` additionally generates a C entry/runtime wrapper and links
both. C implements argv parsing, output, errors and timing, not NIL interpretation.

## Semantics and lowering invariants

Each semantic value maps to a typed LLVM operand. Every basic block has exactly one
terminator; conditional results merge through typed phi nodes. Loop headers have
one phi per state value; backedges contain the simultaneous update results.
Branch/condition/body/finish regions preserve HIR scopes and evaluation order.
Clang verifies the emitted module while compiling it. No unchecked HIR enters codegen.

Checked add/sub/mul use LLVM signed overflow intrinsics. Division checks both zero
and MIN/-1 **before** sdiv. Inactive branches cannot trap. Bool maps to i1, integer
to i64. Internal bool-returning HIR functions are supported; native CLI entry
parameters/results are i64, matching current source signatures. No float, aggregate,
memory or plugin features are added by this backend.

Bounded instrumentation uses a per-invocation context holding fuel, call depth and depth limit. Fuel charges each
executed HIR instruction and region/function yield, exactly as the reference evaluator.
Bounded limits default to 100,000 steps and 256 function frames; region nesting does not
consume call depth. Depth 0 and fuel 0 fail deterministically. Native library bounded build
options allow depth 0..256. Context initialization occurs on every entry invocation;
there is no shared language state or heap allocation for region/loop values.
Select `--bounded` or `--unbounded` before run/build/llvm to override accounting.
Earlier profiles default to bounded; v3 defaults to unbounded. E008/E009 codes
and original byte spans are preserved when their corresponding checks apply. Native traps print a diagnostic
and exit 1; argument syntax errors exit 2. No recoverable native library error ABI is
promised yet. Backend/toolchain failures use E011 with phase Backend.

## Performance and limits

LLVM O2 is the default; no NIL-specific optimization passes exist. `--bench COUNT`
on a generated executable times COUNT calls after 1,000 warmups and reports result
plus ns/call JSON. Timing excludes argv parsing, process startup and compilation;
each call resets its context and checks its result. It is a benchmark interface.

[Recorded results](../../benchmarks/paired/results/2026-09-30/NATIVE_LLVM.md) separate
frontend, SSA/IR lowering, LLVM codegen, C runtime compilation, linking, binary size
and execution. Native runtime gains do not imply cheaper compilation or better LLM
repair/TCR. No compile cache, JIT, bundled LLVM, cross-compilation, portable MIR ABI,
custom optimizations, plugin ABI or full M4/M10 completion is included.

See [ADR 012](../adr/012.md). Run `./scripts/ci.sh` and `./scripts/ci.sh release`:
native O0/O2 differential tests cover all current operations, boundaries, lazy
branches, nested regions, recursion, exact fuel/call limits and malformed arguments.
