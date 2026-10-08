# Round 23 — ledger admission stopped on baseline diagnostic divergence

**B1 did not ship.** The untouched reference evaluator returns E015 while native
O0/O2 return E013 for the same partially failing file read, at the same source
span. Preserving both starting outcomes and requiring reference/native agreement
cannot both hold. No ledger, G2 lift, swap/pop, limit change or ownership change
was implemented. This is the goal's B1 stop condition, not a claim that logical
accounting is impossible. Error precedence needs a separate explicit decision.

Branches are local `feat/core-memory` from `7c48c4a` and `exp/core-memory` in the
benchmarks submodule from `ec505b3`, without upstreams. **Nothing was pushed.**
[ADR 045](adr/045.md), preregistration commit `ca82799`, precedes the benchmark
harness (`683904c`) and all committed Round 23 implementation artifacts. There are
no compiler implementation commits.

## Concrete counterexample and causal evidence

```text
=large=!bytes(67108500,0);n=#!read("nil-quota-fault.bin");#large+n
```

The authored fixture is 4096 `x` bytes. A deterministic test-only interposer makes
reads at offset 4096 return EIO. The fault offset and fixture are identical for
Rust `File::read` and C `fread`. The reference uses the actual `FileHost`; neither
compiler nor runtime is patched. The native reader requests different amounts,
which is precisely what exposes the two error priorities.

The live byte value charges 67,108,540 B, the 19-byte path charges 59 B, and the
prospective read header reserves 40 B. At the read, only **225 payload bytes** fit
under 67,108,864 B. Native reads 226 bytes and rejects them before requesting EIO.
Reference gathers 4096 bytes, then receives EIO before admitting its read result.
`FileHost` caps reads at 64 MiB + 1; the fault here is well below that cap.

| Execution | Transferred prefix | EIO returns | Diagnostic |
|---|---:|---:|---|
| Reference | 4096 B | 1, on a nonzero-size request | E015 @29..30 |
| Native bounded O0 / O2 | 226 B | 0 | E013 @29..30 |
| Native unbounded O0 / O2 | 226 B | 0 | E013 @29..30 |

All emit zero stdout. This is a **fault-injected read failure**, not an observed
disk failure, measured performance result or invented model/runtime output.

The fixed matrix includes seven conflict families: retained Bytes, Buffer,
record child, record-buffer child, map alongside bytes, selected lazy arm and
callee with a retained argument. Seven controls cover errors without quota
pressure, a real successful oversized read, an error after 64 bytes but before
quota, an unselected arm, a dead allocation, a real short successful file, and an
error before any prefix. All controls agree, including exact output and spans.

Three repetitions: **14 fixture families over 10 distinct sources, 42 reference runs, 168 native
comparisons, 84 diagnostic divergences**. Native sanitizer builds add 14 reference
runs / 56 comparisons / 28 divergences, with no sanitizer finding. This is one
error-ordering defect reproduced through seven shapes, not 84 different bugs.

The first exploratory hook was rejected: forwarding public `read` through
`dlsym` recursed in Rust, and public-read interposition missed Darwin libc's
internal reader. The final fixture uses a kernel read for unrelated Rust reads
and a controlled size-one `fread` adapter with Darwin header-defined error/EOF
flags. Positive controls and byte/EIO receipts verify the adapter. It is macOS
specific, is never linked into NIL, and makes no portable-platform claim.

## Exact admission command

From the repository root:

```sh
cargo build --release --locked --offline -p nil -p nil-compiler -p nil-hir
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/core-memory-study -p 'test_*.py' -v
python3 benchmarks/core-memory-study/audit_read_order.py \
  --out /tmp/nil-round23-admission --repeats 3 --require-parity
```

Actual admission output (exit **1**):

```text
TOTALS {"families": 14, "conflict_families": 7, "reference_runs": 42, "native_runs": 168, "comparisons": 168, "diagnostic_divergences": 84}
ADMISSION BLOCKED_B1
```

The [reproducer documentation](../benchmarks/core-memory-study/README.md) describes
isolation, fault injection and controls. Without `--require-parity`, exit 0 means
an expected-bug audit succeeded, **never** that differential parity passed.
Unexpected crashes, diagnostics, spans, output or fault traces fail closed.

## Baseline gate results, not ledger validation

Raw command output and JSON remain outside git at
`~/.local/share/nil/round23-memory/`. The unchanged Rust compiler passes its
existing coverage; that does not erase the failed new admission experiment.

| Command | Actual result |
|---|---|
| `./scripts/ci.sh` | exit 0; 526 tests; strict Clippy; zero warnings |
| `./scripts/ci.sh release` | exit 0; 526 tests; zero warnings |
| `./scripts/sanitize.sh` | exit 0; 237 tests; all listed suites; zero findings |
| Study `unittest discover` above | exit 0; 5 harness tests |
| `./scripts/fuzz-v5.sh --seed 23001 --out …/fuzz-23001` | exit 0; 673 programs; 1346 native builds; zero divergences |
| Same, seed 23002 | exit 0; 673 programs; 1346 native builds; zero divergences |
| Application `verify.py --output …/corpus.json` | exit 0; 68 NIL programs; 2815 checks; 11 unsupported |
| Same `--group original --output …/original.json` | exit 0; exactly 610 checks |
| NeetCode `verify.py --verified-only --jobs 1 --output …/tuned.json` | exit 0; 29 programs; 1070/1070 checks |
| Same `--candidate-dir "$PWD/benchmarks/core-generalize-study/natural" --output …/natural-isolated.json` | exit 0; 29 programs; 1070/1070 checks |

CI counts sum the built-in test runner's `test result` counts, including doctests.
Application totals: reference 563, native 1126, Python 563, C++ 563. Original:
reference 120, native 240, Python 125, C++ 125. No frozen inputs were altered.

The first natural cohort run retained **1069/1070** checks: its largest heap
reference timed out at the unchanged 120-second harness limit while other gates
were running. Native O0/O2 passed that fixture. After other gates finished, the
same entire cohort ran alone with no source or timeout changes and passed
1070/1070, including the heap. Both reports/logs are retained; the initial timeout
is not silently relabeled as success or a language diagnostic.

Both regular fuzz seeds execute every family index exactly once, compare codes
**and spans**, stdout/files/effect ordering, and retain failure seeds. Both have
665 distinct source hashes; 583 are common, union 747. Constants vary in many
otherwise fixed shapes; two seeds are not a general structural proof.

| Reference outcome | Seed 23001 | Seed 23002 |
|---|---:|---:|
| OK | 500 | 500 |
| E008 | 0 | 0 |
| E009 | 8 | 8 |
| E012 | 27 | 27 |
| E013 | 38 | 38 |
| E014 | 10 | 10 |
| E015 | 8 | 8 |
| E016 | 14 | 14 |
| E017 | 10 | 10 |
| E018 | 11 | 11 |
| E019 | 3 | 3 |
| E020 | 6 | 6 |
| E021 | 35 | 35 |
| E022 | 3 | 3 |

Each seed reports all 491 call-family tags (618 tagged instances); the raw logs
contain every per-family count and all 673 index counts. The ordinary campaign
is unbounded and observes **no E008**; existing bounded-depth/instruction tests
remain in CI. The normal generator has no partially failing stream Host family.
Its zero divergences are therefore compatible with the new audit's real failure.

## Natural/tuned counts, census and performance

**Carried-forward qualification is natural 19/150, tuned 22/150.** No new timing
qualification is claimed. Both correctness cohorts were reverified. The 114
held-out problems stay unattempted. No compiler optimization was admitted; no G2
fallback was lifted, and no G1 provenance proof was added.

The Round 22 physical-copy census remains the baseline: natural REQUIRED
1,994,680 events / 1,581,298,251,755 B, AVOIDABLE 3,896,227 / 160,056,657,866 B;
application REQUIRED 2017 / 26,310 B, AVOIDABLE 6017 / 131,303 B. These are
**previous measurements**, not a fresh physical/logical census or an AFTER run.
All scopes, five-cliff min/median/max, RSS and controls remain available in the
[Round 22 report](../benchmarks/reports/2026-10-08/ROUND22.md). No new time/RSS
numbers are invented, extrapolated, or inferred from concurrent verification.
B1 failed admission before timing/census of a proposed ledger was meaningful.

Git tree identity was checked against `7c48c4a` for `crates/`, `cli/`, `tools/`,
`std/`, `scripts/`, `examples/`, `fuzz/`; all are unchanged. Benchmarks trees
`corpora/`, `core-generalize-study/natural/`, `generation/`, `helpers/` match
`ec505b3`. Intrinsic count stays **35**, executable surface unchanged.

## Conditional std/limit plans and next decision

Before code, tiktoken 0.12.0 measured swap/pop in both cl100k and o200k:
`!swap(a,b,c)` 6 versus equivalent swap 9; `!pop(a)` 4 versus slice 10;
`!remove_last(a)` 5. These are forecasts, not shipped spellings or realized corpus
savings. Reviewing the 29 frozen natural sources finds one direct exchange
candidate, the heap in Last Stone Weight; no direct remove-last slice. Other
chained replacements are not automatically swaps. No source was rewritten.

ADR 045 conditionally re-registers 256 MiB / depth 1024 for B4 from Round 21's
RSS/headroom evidence. **No defaults changed:** memory is 64 MiB; reference and
bounded native depth are 256. No additional unbounded-native stack safety is
claimed. B2–B4 forecasts are untested, not forecast hits or hidden misses.

Choose the host error contract before resuming B1: retain reference's read-result
priority (which changes native prefix behavior), or retain native's early-prefix
quota priority (which needs reference/Host support for the prefix information).
A separate explicit decision must describe the changed observation and its
regressions. This report does not choose one or authorize weakening quota/effects.
The ledger's alias/child/death proof is still required after that decision.

## Output provenance

Raw logs, reports, binaries, sources and injected-reader receipts stay outside
git in `~/.local/share/nil/round23-memory/`. The admission reports contain all
commands and native build commands, tool versions, compiler/fixture/source
hashes, and per-run codes/spans and fault receipts. `final-admission/report.json`
is the final ordinary matrix; `ledger-admission-sanitized/report.json` records
the sanitizer matrix. Both compiled reference and fault-injector source hashes
match the committed files. `manifest.json` hashes the root gate logs/reports;
each admission report also hashes its complete per-run artifact directory.
The successful isolated cohort and the initial timeout are both archived.
