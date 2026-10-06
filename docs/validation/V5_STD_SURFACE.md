# Round 14 — source std, measured retention decisions

## Unfavourable findings first

**Zero of three attempted intrinsic migrations passed the placement rule.** Find,
has and parse stay executable core operations with unchanged spellings and
semantics. Source candidates cost 2.84×, 2.10× and 1.13× their core medians,
respectively. Find and has introduce new E013 quota failures; parse still calls
core parse to raise E016, so it is not a complete migration. No rejected wrapper
is presented as a successful migration.

New std filter costs approximately **480 ms at 16 MiB**, matching its handwritten
loop but not establishing C++-class performance. Repeated per-element allocation
and concat bookkeeping remain; this round makes no filter-versus-C++ claim.
The callee-update copying cliff remains deliberately unfixed. Coverage improved,
but many fuzz structures remain fixed across seeds; equal diagnostic totals are
not evidence of independent random programs.

## Decisions and surface

[ADR 035](../adr/035.md) and the [ADR 036 draft](../adr/036.md) were committed in
`e5ec6ce` before implementation `60420a8`. ADR 035 supersedes ADR 026's placement
rule: move operations only when same-name token cost, complete observable
semantics and measured speed all pass. Type/storage/element/scalar primitives and
host authority remain core. Ordinary body-derived proofs still apply.

`std/bytes.nil` and `std/buffer.nil` are pure NIL, embedded/versioned with the
compiler, loaded only when referenced, parsed, specialized and validated through
the existing compiler/module path. There is no search path, trust annotation,
program import, flag or manifest for std. Named std cannot be shadowed by external
modules; unknown names are errors. Existing zero-argument `!map()` remains a map
constructor; two-argument map is the algorithm. Earlier profiles/default retain
meaning; corpus sources and goldens were not edited.

Third-party qualification `!ID.EXPORT(args)` and callback `&!ID.EXPORT` resolve the
same manifest exports as existing `!plugin`; they do not auto-load third parties.
Multiple IDs stay disjoint. Canonical numeric IDs avoid a new naming layer.

Tiktoken 0.12.0, identical counts on cl100k and o200k:

| Source | cl100k | o200k |
|---|---:|---:|
| `!sort(a)` | 4 | 4 |
| `!find(a,10,b)` | 7 | 7 |
| `!map(&c,a)` | 6 | 6 |
| `!filter(&c,a)` | 6 | 6 |
| `!fold(&c,a,0)` | 8 | 8 |
| local `b(&c,a)` | 5 | 5 |
| old `!plugin(7,0,a)` | 8 | 8 |
| new `!7.0(a)` | 6 | 6 |
| old `!plugin(7,12,a,b)` | 9 | 9 |
| new `!7.12(a,b)` | 7 | 7 |
| considered `!7:0(a)` / `!7/0(a)` | 6 | 6 |
| hypothetical named alias `!m.a(a)` | 5 | 5 |

Implemented numeric dot meets the two-token gate without new manifest bindings.
Same-name std calls cost exactly the intrinsic spelling; std loading costs zero
source tokens. Forecast 1.00–1.10× for map/filter/fold matches measured ratios
1.001/1.004/0.997. Find/has/parse forecasts were conditional on preserving primitive
contracts; the failed conditions are reported rather than overwritten.

## O2 performance and disassembly

Apple M2 arm64, Clang 21, existing LTO and checked execution. One warm-up, **25
rotating repetitions**, cached input, whole-process wall time including startup,
read and stdout capture. Every timed output is byte-exact; verification outside
the timing. No CPU isolation or general confidence interval. Large maxima expose
scheduler noise; small median differences do not establish speedups.

Map/filter materialize output (including captured stdout); fold returns a scalar.
Pairs implement the same loop, callback and effects. All sizes below are 16 MiB
except has/parse, which execute 100,000 calls with alternating inputs. Find is a
no-match full scan; equality compares two distinct equal 16 MiB allocations.

| Operation/control | Min / median / max, ms |
|---|---:|
| map_direct | 44.638 / 45.253 / 47.171 |
| map_std | 44.791 / 45.304 / 46.296 |
| map_module | 44.640 / 45.181 / 45.983 |
| filter_direct | 475.681 / 478.297 / 878.066 |
| filter_std | 476.937 / 480.332 / 495.242 |
| fold_direct | 5.528 / 5.781 / 7.101 |
| fold_std | 5.397 / 5.761 / 8.040 |
| find_core | 5.050 / 5.235 / 5.616 |
| find_candidate | 14.313 / 14.861 / 17.246 |
| has_core | 4.671 / 4.785 / 7.244 |
| has_candidate | 9.690 / 10.041 / 19.103 |
| parse_core | 10.760 / 11.037 / 14.393 |
| parse_candidate | 12.299 / 12.520 / 20.285 |
| equal_mirrored | 7.194 / 7.566 / 8.758 |
| scan_inline | 7.068 / 7.402 / 9.784 |
| scan_hof_local | 7.021 / 7.347 / 16.161 |
| scan_hof_module | 6.991 / 7.276 / 12.235 |

The cross-module materialized map and the exact Round 13 map-reduce scan both
reach caller/local parity. Scan inline/local/module entry disassembly contains
`ldp q` and `cmeq.16b`; direct/std fold contains vector loads and `uaddw.2d`.
Materialized map/filter are not vectorized in either caller or std form. Core find
calls optimized libc `memchr`; the rejected source candidate is scalar. No claim
that its vectorization survived is made.

Mirrored provider ID 8 (different from shipped equality) now emits an actual
`call i1 @nil_bulk_compare`, reaching 7.566 ms versus the reported 16.57 ms
fallback. The matcher normalizes commutative Equal and Add nodes, including flipped
length/element comparisons and `1+c`; it does not normalize ordered comparisons
or discard dead trapping reads. Mirrored Bytes/Buffer positives and incorrect
stride/asymmetric comparison negatives have regression tests. Std map/filter/fold
do not introduce a new bulk recognizer.

## Why candidates stay core

`candidate_audit.py` runs 15 prototype cases at both O0/O2, including canonical
parse extrema/failures and find trap priority. Two explicit quota counterexamples
occur at both levels:

- Find with a live 67,108,824-byte value: core returns `67108824`; source E014 guard
  constructs a zero-length Bytes object with a header and raises E013 instead.
- Has with a live large value and 512-byte key/needle: core returns `67107409`;
  source key iteration copies the owned key and raises E013.

The source parse candidate preserves tested output/codes but calls
`!parse("invalid")` for E016, leaving core dependency. No new checked-trap or borrowed
key operation was authorized to force a migration. These are observed primitive
contract blockers, not just aesthetic concerns.

## All 35 intrinsic classifications

Enum count **35 → 35**; executable core implementation count **34 → 34**, because
equal already executes provider source. New std algorithms add no intrinsic.

| Intrinsics (each named) | Verdict and reason |
|---|---|
| buffer, bytes, map, bytemap | Stay core: typed storage construction |
| get, key, size | Stay core: primitive element/metadata access and owned projections |
| i64, u64, u128, f64, trunci64, truncu64, bits, floatbits | Stay core: scalar/bit representation and conversions |
| read, write, out, env, random, directory | Stay core: host authority and effects |
| find, has, parse | Retained core this round: measured speed/semantic blockers above |
| concat, slice, insert, put, sort | Move only after callee-update reuse; sort additionally needs a suitable algorithm |
| parsebuf, format, parseu64, parseu128, parsef64 | Later measured migration: checked numeric/text and ownership contracts |
| equal | Already outward; generic structural fast path hardened |

## Part C — exact copying diagnosis, no ownership change

Comparator-sort HIR function 3 reads original `v0[v1]` into v4, replaces original
v0 at `v3=b-1` into v5, **then reads original v0[v7=b-1]** into v8, and replaces v5
at v1 with v8 into v9. Both replacements emit `@nil_set`; neither emits unique set
inside this callee. The first copy preserves that later original read. The second
has a dead intermediate but does not acquire loop_storage's deferred replacement
chain proof in an ordinary callee. Its caller's loop performs a normal allocating
call with caller roots retained; return type alone grants no transfer. The fill
loop does emit `nil_set_unique`, isolating the missing callee fact.

A finite origin-set fixed point across validated HIR calls/loop states/fields
counts **52 candidate static call sites in 18 of 68 programs**. A candidate returns
an owned parameter updated by Replace/Concat/Put/Insert; fresh builders are
excluded. It is not dynamic frequency, physical-copy count or a uniqueness proof.
This census is scoped to those four update operations; it is not an exhaustive
count of all allocating callees (for example slice/sort wrappers). B3's three
instantiated std bodies contribute **0/6** such calls; unchanged sort
has **3/5**. A first coarse census included fresh builders and was replaced by this
stricter lineage analysis before reporting.

ADR 036 remains **draft**, not accepted: caller last-use, descendant aliases, old
reads, trap/effect ordering and root/quota handoff must all be proved before any
ownership transfer. Removing per-swap whole-buffer copies would remove copy
amplification, not insertion sort's O(N²) comparisons. No defensible absolute
milliseconds forecast exists until that proof is prototyped.

## Fuzz coverage, diversity and defects found

438 families (418 prior + 20 added). Campaigns reject `--cases < 438` before
creating artifacts; the explicitly named CI smoke API is partial and is not the
CLI campaign. `--cases 256` now fails loudly. The 438 indices collapse to 383
named call-family buckets; neither count is hidden. Each of seeds 5130572, 1729 and
4294967295 runs every index once: **438 programs / 876 O0/O2 builds / zero
divergences per seed**. Host stdout and file effects are compared with the
in-memory reference Host, not merely returned scalars.

| Result code | Count per seed |
|---|---:|
| OK | 317 |
| E009 | 3 |
| E012 | 20 |
| E013 | 31 |
| E014 | 7 |
| E015 | 8 |
| E016 | 14 |
| E017 | 10 |
| E018 | 11 |
| E019 | 3 |
| E020 | 6 |
| E021 | 5 |
| E022 | 3 |

Per-index coverage and full per-family/code maps are recorded in each fuzz output.
New named families execute once each per seed: std_map_bytes, std_map_buffer,
std_filter_bytes, std_filter_buffer, std_fold_buffer, std_fold_bytes, std_empty_map,
std_empty_filter, std_lazy_unselected, std_effect_order, std_callback_quota (E013),
std_byte_range (E014), find_retained_core, has_retained_core, parse_retained_core,
parse_failure (E016), mirrored_equal_alias, mirrored_equal_length,
mirrored_equal_buffer, std_qualified_module. All others in this list return OK.
Retained operations are deliberately labelled core, not successful std migrations.

Exact-source FNV-1a hashes: **430 distinct/seed; 556 across all seeds; 363 common to
all**. Pairwise common/symmetric-difference: 367/126, 364/132, 366/128. This is a
noncryptographic diversity indicator. Many edge families are fixed and others vary
constants; no claim of 1,314 independently random structures. Source mutation
cases exercise the new spellings/signatures as well as valid programs. The HOF
mutation checks yield E002:46, E004:46, E005:46, E007:92 per seed; these static
diagnostics are separate from the execution-code table above.

Implementation review found and fixed a nested named-call parser stack exhaustion
(regression: depth 512 returns E001), a callback type-discovery ordering error
under each, and a potential panic for malformed embedding AST record IDs
(regression E023). No native sanitizer finding or differential divergence was
observed. Prototype authoring mistakes were corrected before final candidate
measurement; they were not compiler bugs.

## Historical controls and unchanged corpus

| Control | Historical median | Current min / median / max, ms |
|---|---:|---:|
| 1 MiB append | 22.709 | 22.066 / 22.534 / 22.979 |
| 16 MiB scan | 7.849 | 7.083 / 7.507 / 7.899 |
| 16 MiB transform | 36.156 | 34.064 / 35.047 / 40.852 |

25 repeats, same existing runtime benchmark methodology. Outputs pass including
byte-exact transform and double-transform round trip. No regression beyond noise.
C++/Python medians respectively: append 4.977/44.640, scan 10.062/23.861,
transform 12.376/806.382 ms; these are distinct from stdout-materialized map controls.
No runtime, arena, quota or backend element-access change was made this round.

Full corpus: **2,815 checks**, comprising 563 reference + 1,126 native O0/O2 + 563
Python + 563 C++; **68 supported/11 unsupported out of 79 tasks**. Separate
`--group original`: **exactly 610** (120 reference, 240 native, 125 Python, 125 C++).
No tasks, reference solutions, baselines, oracles or goldens changed. All per-task
source hashes, bytes, characters, token records and cut aggregates equal the
Round 13 measurement exactly on Gemma, Qwen, cl100k and o200k. No density improvement
or new model-efficiency result is claimed.

| Cut | Gemma NIL / Python / C++ | Qwen NIL / Python / C++ | cl100k NIL / Python / C++ | o200k NIL / Python / C++ |
|---|---:|---:|---:|---:|
| original | 2161 / 2305 / 5642 | 1646 / 1786 / 4378 | 1594 / 1763 / 4360 | 1583 / 1770 / 4381 |
| new_external | 11297 / 8280 / 17485 | 8738 / 6765 / 13954 | 8524 / 6684 / 13908 | 8514 / 6710 / 13977 |
| combined | 13458 / 10585 / 23127 | 10384 / 8551 / 18332 | 10118 / 8447 / 18268 | 10097 / 8480 / 18358 |

The original cut includes the Round 13 HOF task (25); the historical original
verification group still preserves exactly 610. External/combined density remains
unfavourable; original-cut results are not evidence of general LLM efficiency.

## Commands and final gate evidence

Gate logs and measured raw data are archived outside version control, following
AGENTS.md. Reproduction tools are committed under `benchmarks/std-study/`. Final
validation runs follow the defensive malformed-AST fix; the fix does not alter
valid-program IR (all 17 timed controls are byte-identical). Earlier measurement
metadata may name the preregistration HEAD while implementation was uncommitted;
this is not claimed to be a clean-revision measurement. Archived sources, IR and
the final comparison bind the measured programs to the committed implementation.

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
for seed in 5130572 1729 4294967295; do
  ./scripts/fuzz-v5.sh --seed "$seed" --cases 438 --out "/tmp/nil-std/fuzz-$seed"
done
benchmarks/paired/.venv/bin/python benchmarks/std-study/naming.py
benchmarks/paired/.venv/bin/python benchmarks/std-study/run.py --repeats 25 --output /tmp/nil-std/performance
benchmarks/paired/.venv/bin/python benchmarks/std-study/candidate_audit.py --output /tmp/nil-std/candidate-audit
benchmarks/paired/.venv/bin/python benchmarks/std-study/copy_census.py --output /tmp/nil-std/census
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-std/corpus.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-std/original.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-std/corpus.json --output /tmp/nil-std/density
./scripts/bench.sh runtime --full --workloads --workload append --workload newlines --repeats 25 --output /tmp/nil-std/historical-controls
./scripts/bench.sh runtime --full --application --repeats 25 --output /tmp/nil-std/historical-transform
```

```text
./scripts/ci.sh                         exit 0; 441 passed; 0 warnings
./scripts/ci.sh release                 exit 0; 441 passed; 0 warnings
./scripts/sanitize.sh                   exit 0; 174 passed; 0 findings
fuzz-v5.sh seed 5130572                 exit 0; 438 programs; 876 builds; 0 divergences
fuzz-v5.sh seed 1729                    exit 0; 438 programs; 876 builds; 0 divergences
fuzz-v5.sh seed 4294967295              exit 0; 438 programs; 876 builds; 0 divergences
verify.py                              exit 0; 2815 checks
verify.py --group original             exit 0; exactly 610 checks
density.py                             exit 0; 237 records and all cuts unchanged
Valid timed IR comparison              17/17 byte-identical after final guard
python -m unittest discover -s benchmarks/tests  exit 0; 10 passed
v5 --cases 256                         expected exit 1; no artifacts created
```

Sanitizers cover all ten selected application suites with ASan, UBSan and
float-cast-overflow instrumentation. macOS leak detection is disabled, as documented
in the script; this is not a LeakSanitizer claim.

Local raw evidence archive (logs, JSON, source fixtures, HIR/LLVM/disassembly;
no executables or large binary inputs):
`/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-std-surface-2026-10-06.tar.gz`

SHA-256: `0f3de55f9d31cd217b05e1e4f42971f6def53422938e65516e868de3f2636887`.

The archive is deliberately outside Git; regenerate it using the committed tools
and commands above. `final-validation.json` binds the gates and compiler/tool
source hashes. Main branch `feat/std-surface` derives from `17f1a5a`; benchmark
branch `exp/std-surface` adds `9830ffb` on `97828c7`. All commits are local; no push.


## Remaining limits and next decision

No successful find/has/parse migration; checked-failure and owned-key primitives
must be addressed or these stay core. Filter parity is not sufficient evidence
of fast filtering. Std overloads cover Bytes/i64 Buffer only, with scalar callbacks
and no captures. Numeric third-party namespaces retain explicit manifests.
Structural matching is still deliberately conservative outside normalized exact
shapes. Callee ownership remains unchanged; comparator insertion sort still copies
and is algorithmically quadratic. Existing conservative shared/nested builder
limits remain. ADR 036 needs a separate sound transfer proof and measurements
before sort/concat/slice/updaters can migrate. No later round starts here.
