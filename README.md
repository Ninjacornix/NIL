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
Floating-point values, exposed memory operations, and plugins remain unimplemented.

## Build and run

Install Rust through rustup; the repository pins the development toolchain in
`rust-toolchain.toml`. Compiling NIL programs also requires Clang 15 or newer,
system C headers, and a linker. On macOS, install Apple's command-line developer
tools. Rust dependencies are confined to this workspace.

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
See [application examples](examples/expr-v5/README.md) and [the memory/effect contract](docs/language/EXPR_V5.md).
It remains experimental; source/repair token efficiency for these features is unmeasured.

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
fresh-local C++ and Python. These measurements do not yet establish whether
NIL reduces total model tokens across generation and repair attempts.

See [CONTRIBUTING.md](CONTRIBUTING.md) for development commands and contribution
guidelines.

## License

NIL is available under the [MIT License](LICENSE).

Benchmark tools and corpora are an optional pinned submodule. Initialize them with
`git submodule update --init benchmarks`; compiler builds and tests do not need it.
