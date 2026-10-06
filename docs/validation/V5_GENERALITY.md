# Corpus-driven v5 generality validation

## Decision and immutable comparison

Ranking commit `5832936` precedes production `abc6afe`, differential generator
`7142629`, and final live-quota/test strengthening `1708ff3`. Benchmark corpus
commit `a6acf16` rewrites eleven references. No shortening experiment, new profile,
Rust dependency, model inference or push occurred. See [ranking](../experiments/V5_GENERALITY_IMPACT.md),
[ADR 022](../adr/022.md), [spec](../language/EXPR_V5.md) and
[before/after tables](../../benchmarks/reports/2026-10-03/V5_GENERALITY.md).

Compared with benchmark `9be44d6`, tasks.json, oracles.py, goldens.json, verifier,
all Python/C++ sources and the original measurement files are byte-identical.
Only eleven `.nil` references, corpus documentation and new reports change.
Compiler manifests, default/earlier profiles and examples remain unchanged.
Reproduce the frozen-artifact check:

```sh
git -C benchmarks diff 9be44d6 -- corpora/application-v5/tasks.json corpora/application-v5/oracles.py corpora/application-v5/goldens.json
# Empty. Also inspect changed paths and source hashes in verification/density JSON.
git diff 5391b3a -- Cargo.toml Cargo.lock examples
# Empty.
```

## Final checks (all exit 0)

```text
./scripts/ci.sh                         220 passed, 0 failed; strict clippy clean
./scripts/ci.sh release                 220 passed, 0 failed; strict clippy clean
./scripts/sanitize.sh                   36 native application tests passed
```

Counts sum all `test result: ok` lines; zero-test/doc targets are included without
inflating the sum. Native tests execute at O0/O2; sanitizer script instruments the
C runtime and linked executables with AddressSanitizer/UndefinedBehaviorSanitizer.
Leak detection follows existing disabled platform policy. These checks ran again
after adding the combined-live-quota case and final generator branch.

Named new tests cover signatures/HIR validation and canonical/binary/extrema parsing;
`new_sequence_operations_match_reference_at_o0_and_o2` asserts exact independent
reference results. `new_sequence_failures_match_codes_and_priority_at_o0_and_o2`
checks E012 before E014, E013 before E016, leading/interior/repeated terminal fields,
canonical failures and combined live caller/input/result capacity. A result that
fits the individual maximum still fails when a live 32 MiB caller alias and 4 MiB
input push total charge over 64 MiB. Reference/native agree.

`new_operations_preserve_lazy_host_effects_and_aliases_at_o0_and_o2` retains selected
arms, left-to-right host effects and caller aliases. Allocation admission precedes
field conversion but follows argument evaluation. No purity/nontrapping attributes
were added to checked operations.

IR tests `equality_search_are_borrowing_but_bulk_parse_retains_roots_and_concat_reuse`
and `parsed_buffer_concat_keeps_copying_when_old_reads_are_live` assert no root frame
for nonallocating search/equality helpers, conservative roots for parsing,
`nil_concat_unique` for a dead append operand, and `nil_concat` with live old reads.
Runtime uniqueness protects caller aliases. Bulk parsing itself constructs fresh
exact-capacity storage; byte input is not reused as an integer payload.

## Differential campaign

```sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 312 --out /tmp/nil-generality/fuzz-5130572
./scripts/fuzz-v5.sh --seed 17 --cases 312 --out /tmp/nil-generality/fuzz-17
./scripts/fuzz-v5.sh --seed 991 --cases 312 --out /tmp/nil-generality/fuzz-991
```

Each run reports 312 programs, 624 native O0/O2 builds and **0 divergences**;
936 programs / 1872 native builds total. The reference uses the in-memory Host.
Diagnostic codes and effects/results are compared, including existing E012–E018.
These are three cycles of 104 randomized scenario families, not arbitrary-program
fuzzing or a proof. Source-appearance intrinsic counts are distinct from executed
error outcomes. Every new operation and failure family is exercised.

| Outcome | Each seed | All three |
|---|---:|---:|
| E012 | 30 | 90 |
| E013 | 12 | 36 |
| E014 | 15 | 45 |
| E015 | 9 | 27 |
| E016 | 27 | 81 |
| E017 | 9 | 27 |
| E018 | 9 | 27 |
| OK | 201 | 603 |

All twelve generated intrinsics: buffer, bytes, concat, equal, find, format, out,
parse, parsebuf, read, slice, write. The 40 named call/alias/lazy families each
occur three times per seed (nine total). Full names/counts and raw summaries are in
[V5_GENERALITY_FUZZ.json](../../benchmarks/reports/2026-10-03/V5_GENERALITY_FUZZ.json).
New families include buffer/byte equality and search, start/range priority,
canonical/empty/binary parsing, allocation priority, lazy calls/argument effects,
caller/loop/recursive/returned aliases, replacement aliases and borrowing leaf scans.

## Frozen corpus and token counts

```sh
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/corpora/application-v5/tests -v
# 5 tests, OK
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-generality/verification.json
# 25 tasks; 24 supported NIL; 120 reference + 240 native + 125 Python + 125 C++ = 610 checks
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-generality/verification.json --output /tmp/nil-generality/density
# Four tokenizers; Qwen cross-check 74 counts, no mismatches; 24 verified positive exports
```

Verification/counting ran on committed compiler `7142629`; later changes only
strengthen tests/generator. All forecasts match verified counts, without modifying
oracles/goldens. Environment lookup stays unsupported, with no invented count.
The tables retain every unchanged task and Python loss. Source-density gains do
not reverse the original generation result (NIL 0/24, Python 11/24); no new inference.

## Performance regression controls

Before checkout `/tmp/nil-generality/before` was detached at `5391b3a`; after was
`7142629`. Accepted runs were sequential after substantial verification/counting
jobs completed, nine medians with one warmup, rotating languages, cached files,
whole-process timing and unchanged workload programs:

```sh
NIL_ROOT=/tmp/nil-generality/before benchmarks/paired/.venv/bin/python benchmarks/bench.py runtime --full --workloads --workload newlines --workload append --repeats 9 --output /tmp/nil-generality/before-workloads
./scripts/bench.sh runtime --full --workloads --workload newlines --workload append --repeats 9 --output /tmp/nil-generality/after-workloads
NIL_ROOT=/tmp/nil-generality/before benchmarks/paired/.venv/bin/python benchmarks/bench.py runtime --full --application --repeats 9 --output /tmp/nil-generality/before-transform
./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-generality/after-transform
```

| Control | NIL before ms | NIL after ms | C++ after ms | Python after ms |
|---|---:|---:|---:|---:|
| 16 MiB newline scan | 7.320 | 7.371 | 9.851 | 23.161 |
| 1 MiB append | 24.176 | 24.153 | 4.692 | 43.103 |
| 16 MiB transform | 39.051 | 37.855 | 14.741 | 785.756 |

All manifests report passed verification: exact append content, arbitrary-byte
scan control and byte-exact/double-transform round trips at 4 KiB, 1 MiB, 16 MiB.
An initial run overlapping corpus verification is excluded, retained in the archive.
No systematic regression appears, but whole-process noise is not an equivalence
proof. NIL still loses to C++ on append/transform. Normalized before/after IR is
identical after removing three unused declarations; hashes and full samples are in
[V5_GENERALITY_PERFORMANCE.json](../../benchmarks/reports/2026-10-03/V5_GENERALITY_PERFORMANCE.json).
These are regression controls, not new-operation runtime benchmarks.

## Findings and raw evidence

No production defect was found by the differential campaigns or sanitizers. An
initial native fixture declared a zero-parameter helper as `0=`; corrected to the
existing `=` grammar. Strict Clippy caught a helper after the test module; moved it
before the module. No oracle/golden was changed to resolve either issue.

Local raw command logs, unverified estimates, accepted/excluded manifests, exported
positives, tokenizer pins and before/after IR are archived outside both repositories:

- Archive: `/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-generality-2026-10-03.zip`
- SHA-256: `d9a50818dccdc3d1291e05632080d1d292b1eed17b2f90770d6098d0e03e58a3`

The archive contains no model tensors, compiler binaries or credentials. Published
JSON/CSV records and executable corpus permit independent reruns; tokenizer assets
must match their existing pins. All commits remain local; no remote branch or
submodule branch was pushed.
