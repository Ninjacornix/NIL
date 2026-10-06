# Ordered map validation — 2026-10-03

## Decision and scope

ADR 023 was committed as `95db914` before compiler changes (`9cf1f43`, `d10ed11`).
Two homogeneous byte-keyed map types share one owned allocation implementation.
No records, recursive maps, new dependencies, unsafe Rust, earlier/default-profile
semantics, quota limit or host/plugin ABI changes. Existing last-use analysis
accounts for every intrinsic argument and unions lazy captures; runtime single-root
uniqueness additionally discharges aliases across callers and loop state. It is
not an assumption that a callee will inline. Copying remains the fallback.

Byte-valued entries contain owned sequences. Map-valued entries remain unsupported;
“nesting” in this campaign means a map containing byte sequences and composed/nested
calls/loops, not arbitrary recursive containers. Different-length byte updates can
repack whole payloads; this is measured rather than hidden behind the fast insert.

## Commands and observed results

Run from the NIL workspace. All commands exited 0 unless an expected E013 boundary
failure is explicitly reported. Full output is archived below.

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
```

Observed sums of the actual `test result: ok. N passed` lines:

```text
ci-debug-complete: 239 passed; no FAILED/error:/warning: lines
ci-release-complete: 239 passed; no FAILED/error:/warning: lines
sanitize-complete: 55 passed; no FAILED/error:/warning: lines
```

The CI script runs rustfmt and strict Clippy. Sanitizers run both the 36 existing
application tests and 19 keyed tests, including native O0/O2. ASan and UBSan cover
both emitted IR and C runtime. Leak detection is disabled on macOS; do not read
this as a leak audit. These names establish the new behaviours:

- `map_integer_lookup_and_update_preserve_order`
- `map_growth_and_hash_collisions_preserve_insertion_iteration`
- `map_keys_compare_all_bytes_including_non_utf8_and_nul`
- `map_owns_key_and_value_bytes_and_lookup_returns_independent_storage`
- `replaced_map_read_after_update_keeps_old_value`
- `caller_alias_survives_callee_map_update`
- `loop_state_map_alias_and_returned_alias_remain_immutable`
- `map_lazy_arms_do_not_evaluate_missing_or_duplicate_keys`
- `map_argument_traps_precede_missing_and_duplicate_checks`
- `every_allocating_map_operation_checks_its_result_charge`
- `duplicate_key_precedes_a_result_quota_failure`

Every new runtime operation has reference/native O0/O2 parity, including missing,
duplicate, iteration and allocation failures. Invalid signatures are checked before
execution. Literal/same-length/different-length byte values, root admission and
empty-map CLI entry arguments are covered. The existing HIR/golden/regression tests
pass; earlier examples were not edited.

## Differential campaigns

```sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 268 --out /tmp/nil-keyed/fuzz-final-5130572
./scripts/fuzz-v5.sh --seed 1729 --cases 268 --out /tmp/nil-keyed/fuzz-final-1729
./scripts/fuzz-v5.sh --seed 8675309 --cases 268 --out /tmp/nil-keyed/fuzz-final-8675309
```

Each observed output reports 268 programs, 536 native builds, **0 divergences**:
804 programs / 1,608 O0/O2 native builds total, using the in-memory Host oracle.
Return bytes (including map insertion order), stdout/file effects and diagnostic
codes are compared. Each seed has these executed result counts:

| Result | 5130572 | 1729 | 8675309 |
| --- | ---: | ---: | ---: |
| OK | 172 | 172 | 172 |
| E012 | 22 | 22 | 22 |
| E013 | 20 | 20 | 20 |
| E014 | 10 | 10 | 10 |
| E015 | 6 | 6 | 6 |
| E016 | 18 | 18 | 18 |
| E017 | 6 | 6 | 6 |
| E018 | 6 | 6 | 6 |
| E019 | 2 | 2 | 2 |
| E020 | 6 | 6 | 6 |

Each seed executes two instances of all 30 map `call_families`: empty/construction,
binary keys, lookup, order/growth, old-read alias, owned nested byte values, byte
replacement alias, missing/duplicate keys, iteration bounds, caller/loop/lazy/returned
aliases, byte repacking, insertion loop, quota boundary, byte-result lifetime,
duplicate argument effect, unselected called effect, recursive alias, byte-value
calls, nested loop, key iteration, both constructor quotas, put/key/get-result
quotas and duplicate-before-quota. Full exact family names/counts and generated
intrinsics: [JSON](../../benchmarks/reports/2026-10-03/V5_KEYED_FUZZ.json).

Generation is 134 seeded templates with varied payloads/keys/lengths, not unrestricted
random map AST/IR mutation. Error counts are reference outcomes, not triple-counted
native runs. No divergence or sanitizer bug was found. Six small regression/corpus
seeds live under `fuzz/corpus/expr-v5/map-*.nil`.

## IR admission evidence

```sh
./target/release/nil --profile expr-v5 llvm benchmarks/application/keyed/insert.nil
./target/release/nil --profile expr-v5 llvm fuzz/corpus/expr-v5/map-alias.nil
```

Actual relevant lines:

```llvm
; Single-use loop:
%v10 = call ptr @nil_map_insert_unique_int(ptr %v2, ptr %v8, i64 %v3, i64 25, i64 26)
; Live old-value read in the helper:
%v3 = call ptr @nil_map_put_int(ptr %p0, ptr %v1, i64 9, i64 32, i64 33)
```

`last_use_map_update_emits_unique_path_and_live_alias_emits_copy` asserts both.
The unique C entry still checks roots==1 and reserves the same semantic charge;
a surviving runtime alias copies. Byte-length changes or capacity growth also copy.
The IR call is admission evidence; measured scaling corroborates actual reuse.

## Corpus and density

```sh
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/corpora/application-v5/tests -v
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-keyed/verification.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-keyed/verification-original.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-keyed/verification.json --output /tmp/nil-keyed/density
```

```text
Ran 11 tests ... OK
TOTALS tasks=75 nil_programs=56 unsupported=19 reference_checks=478
       native_checks=956 python_checks=483 cpp_checks=483       [2,400]
TOTALS tasks=25 nil_programs=24 unsupported=1 reference_checks=120
       native_checks=240 python_checks=125 cpp_checks=125       [610]
Adaptation examples: 56
QWEN_CROSSCHECK comparisons=170 mismatches=[]
```

Original contracts/sources remain hash-locked. KEYED_LOCK.json additionally checks
all frozen external contracts and existing baseline sources. Only one existing NIL
source is rewritten (grade_school); new ETL references are added. SUPPORT.json
updates support without modifying tasks/oracles/goldens. Recorded unsupported
attempts preserve every task; ETL alone changes status. No baseline was shortened
or altered to improve the comparison.

[Three cuts, every task, four tokenizer versions, bytes/chars and confidence limits](../../benchmarks/reports/2026-10-03/V5_KEYED_DATA.md).
The external Python gap persists at 12.0–21.1%; combined 6.9–14.6%. The original
24 counts are unchanged. The initial ADR helper estimate used wrong line indexes;
its correction is disclosed, with the original forecast preserved in history.
No positive model-efficiency finding follows from source density.

## Performance and boundary

```sh
benchmarks/paired/.venv/bin/python benchmarks/application/keyed/run.py --output /tmp/nil-keyed/scaling-final --repeats 9
./scripts/bench.sh runtime --full --workloads --workload newlines --workload append --repeats 9 --output /tmp/nil-keyed/controls-workloads
./scripts/bench.sh runtime --full --application --repeats 9 --output /tmp/nil-keyed/controls-transform
rustc --edition=2024 -O benchmarks/helpers/keyed_boundary.rs --extern nil_compiler=target/release/libnil_compiler.rlib -L dependency=target/release/deps -o /tmp/nil-keyed/keyed-boundary-reference
/tmp/nil-keyed/keyed-boundary-reference benchmarks/application/keyed/insert.nil
```

```text
insert 1k / 4k / 16k / 64k: 2.497 / 2.924 / 4.195 / 10.219 ms
repack 1k / 4k / 16k / 64k: 2.630 / 2.845 / 6.786 / 72.768 ms
repack-held 1000 updates, 4KiB / 64KiB / 1MiB: 3.772 / 4.109 / 21.431 ms
reference and native decimal-key builder:
262144: OK 262144
262145: OK 262145
262146: E013 @25..26 dynamic allocation limit exceeded
16MiB scan: NIL 7.476 / C++ 9.959 / Python 23.681 ms
1MiB append: NIL 24.088 / C++ 4.805 / Python 43.189 ms
16MiB transform: NIL 38.117 / C++ 14.666 / Python 771.726 ms
```

Whole-process medians of nine, one warmup, compilation excluded, outputs verified.
The controls run after CI/fuzz/sanitizers, without concurrent measurement jobs.
Map scaling includes startup/key formatting; low-size startup obscures proportional
work. Repack includes fresh large arguments; repack-held exposes unrelated-payload
copying independently. Neither result claims all map updates are constant time.
The previous control medians (7.371/24.153/37.855) are historical, not a paired
before replay. Small changes are inconclusive; no systematic regression claim.
Transform is byte-exact and double-transform round-trips; append content and arbitrary
byte scan controls pass. [All samples and measured sizes](../../benchmarks/reports/2026-10-03/V5_KEYED_PERFORMANCE.json).

## Evidence archive and local commits

Full command logs, IR snippets, verified export and raw measurement outputs:
`~/.local/share/nil/benchmarks/archive/v5-keyed-data-2026-10-03.zip`.
SHA-256: `9174d704c9c72dc084c4c37cc73ec96212d432c70b09a55adc2fc52e42d6a3c7`.
Generated binaries and fuzz artifacts remain outside Git. The benchmark submodule
contains reference sources, runner and measured JSON/CSV reports. Both repositories
use logical local Conventional Commits; nothing was pushed.
