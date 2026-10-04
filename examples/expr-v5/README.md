# Application examples

Build with `cargo build --release -p nil --locked --offline`.

```sh
./target/release/nil --profile expr-v5 run examples/expr-v5/greet.nil 0 World
# Hello, World
./target/release/nil --profile expr-v5 run examples/expr-v5/sum.nil 0 '[1,2,3,4]'
# 10
./target/release/nil --profile expr-v5 run examples/expr-v5/buffer.nil 0 300
# 300 copies of 3, printed as an integer list
./target/release/nil --profile expr-v5 run examples/expr-v5/uppercase.nil 0 'Hello, NIL!'
# HELLO, NIL!
./target/release/nil --profile expr-v5 run examples/expr-v5/parse.nil 0 '-42'
# -42
./target/release/nil --profile expr-v5 run examples/expr-v5/integer_file.nil 0 number.txt
# reads a decimal integer, allowing one trailing newline
./target/release/nil --profile expr-v5 run examples/expr-v5/copy.nil 0 input.bin output.bin
# byte count; output.bin contains an exact binary copy
./target/release/nil --profile expr-v5 run examples/expr-v5/file_size.nil 0 input.bin
# file size in bytes
./target/release/nil --profile expr-v5 run examples/expr-v5/transform_file.nil 0 input.bin inverted.bin
# byte count; inverted.bin contains 255 minus each input byte
./target/release/nil --profile expr-v5 build examples/expr-v5/copy.nil -o /tmp/nil-copy
/tmp/nil-copy input.bin output.bin
```

`line_count.nil` counts newline bytes in a text argument. `uppercase.nil` changes
ASCII lowercase bytes only; UTF-8 multibyte text is preserved, not Unicode-cased.
Byte results are written directly, followed by a result newline. File writes use
the caller's permissions and truncate the target. Proven dead replacement chains
reuse storage; aliases force copying. Live operands and transient results still
share a 64 MiB budget. See [v5 semantics](../../docs/language/EXPR_V5.md).

`record.nil` passes a nested immutable record through a typed helper:

```sh
./target/release/nil --profile expr-v5 run examples/expr-v5/record.nil
# 42
```

Records can contain existing sequences/maps. Scalar-record map values use
`map[Point]` and `!map[Point]()`; sequence-bearing record map values remain
unsupported. Dynamic record buffers use `v[Point]` and
`!buffer[Point](length,fill)`. Use a scalar/sequence CLI entry wrapper for records.
