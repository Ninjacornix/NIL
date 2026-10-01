# Published benchmark findings

The dated reports preserve historical methods and measured outcomes. Raw JSON links
resolve to the immutable expr-v4 commit preceding benchmark cleanup, so archived
claims remain inspectable without carrying repeated samples in the current tree.
The source hashes in those reports identify their historical compiler, not today's
checkout; rerun through `scripts/bench.sh` for current results.

The 2026-10-01 cleanup also preserved verified local copies of every raw result and
interrupted model artifact under `~/.local/share/nil/benchmarks/archive/`.
Those private local files are not required to build, test or reproduce the fixtures.

- [expr-v4 storage and token results](2026-09-30/EXPR_V4_STORAGE.md)
- [expr-v3 integer core](2026-09-30/EXPR_V3.md)
- [Control-flow source comparisons](2026-09-30/CONTROL_FLOW.md)

Future published studies should retain small summaries here and attach full raw
artifacts to a durable archive. CI smoke artifacts are short-lived, not publication
archives. Never claim interrupted or mock runs as model-efficiency evidence.
