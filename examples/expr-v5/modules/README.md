# Shared application reporting

Both entry files use `reports.nil-module`, loaded by the existing plugin loader.
The library exports newline counting and integer summation; its formatter is private.
These are alternate implementations of the unchanged corpus tasks, not new tasks.

```sh
cargo build --release -p nil --locked --offline
printf 'a\nb\n' > /tmp/module-input
./target/release/nil --profile expr-v5 --module examples/expr-v5/modules/reports.nil-module run examples/expr-v5/modules/count_newlines.nil 0 /tmp/module-input /tmp/module-output
# 2 (bytes written)
cat /tmp/module-output
# 2
printf '20\n22\n' > /tmp/module-input
./target/release/nil --profile expr-v5 --module examples/expr-v5/modules/reports.nil-module run examples/expr-v5/modules/sum_integers.nil 0 /tmp/module-input /tmp/module-output
# 3 (bytes written)
cat /tmp/module-output
# 42
```

Local positional names are private to each file. `!plugin(7,0,...)` names export 0
of the explicitly loaded module 7. No import changes the default profile or entry.
The matching corpus alternatives live under `benchmarks/corpora/application-v5/modules/`.
