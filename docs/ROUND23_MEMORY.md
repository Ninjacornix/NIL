# Round 23 — status and stack-safety admission failure

**Round 23 is incomplete. B1–B3 are gated; B4 has not shipped.** The new
admission audit reproduces unbounded O0/O2 crashes and also finds a bounded O0
stack overflow with a large record parameter. ASan confirms the latter. Adding
an unbounded function-entry counter alone would leave this defect unfixed; no
partial compiler fix was committed. All commits are local; nothing was pushed.

## B4: evidence and stopping boundary

ADR 046's dated admission addendum was committed as `48900fb` before the new
audit. The benchmark audit uses the existing compiler/evaluator/backend APIs,
not a simulated evaluator or modified runtime. Three independently authored
sources exercise the reported 200,000-depth sequence recursion, mutual sequence
recursion, and recursion carrying a 3,840-slot record containing fifteen
256-element arrays. Limits are 32, 256 and 1024; builds are bounded/unbounded,
O0/O2. Each reference run returns E008 with an exact span.

```text
python3 benchmarks/core-memory-study/audit_depth.py --out /tmp/nil-depth-admission-new
11/36 exact; 25 failed; exit 1
19 native signal-11 crashes; 6 bounded depth-1024 build rejections
```

| Case | Bounded at 32 | Bounded at 256 | Bounded at 1024 | Unbounded at all three limits |
|---|---|---|---|---|
| Reported literal recursion | O0/O2 exact E008 | O0/O2 exact E008 | O0/O2 rejected | O0/O2 signal 11 |
| Mutual sequence recursion | O0/O2 exact E008 | O0/O2 exact E008 | O0/O2 rejected | O0/O2 signal 11 |
| Large-record recursion | O0/O2 exact E008 | O0 signal 11; O2 exact E008 | O0/O2 rejected | O0/O2 signal 11 |

Large-record O0 machine-object measurements: entry frame **32,944 bytes**;
recursive callee frame **63,952 bytes**. O2 gives 112 and 2,272 bytes respectively.
These are actual generated machine frames, not sums of LLVM allocas, and are
**not a complete linked stack bound**. Existing `nil_enter` executes after the
callee prologue. At depth 256 the O0 stack can exhaust before that check; the
same source's reference and bounded O2 report `E008 @13903..13911`.

The large-record bounded O0 build with
`-fsanitize=address,undefined,float-cast-overflow -fno-sanitize-recover=all
-fno-omit-frame-pointer` reports **AddressSanitizer: stack-overflow**, then signal
6. This is an admission failure, never a clean E008 or E013 inconclusive. No
other sanitizer finding is inferred from it.

The unbounded emitter also erases source-call boundaries via linear HIR inlining
and scalar read replay. A correct depth implementation must preserve those
logical calls or prove that its configured limit cannot be reached. Native safety
needs a private execution stack with a verified bound and checks before large
prologues, covering generated helper chains, record-child recursion, entry
marshalling and sanitizer frames. That complete bound has **not been established**.
ADR 046 explicitly requires stopping at the last completed step in that case.
No guessed stack size, broad signal-to-E008 conversion, or weakened alias proof
was shipped. This is an unfinished implementation obligation, not evidence that
safe native execution is impossible.

COV-1 remains **unmet in the default differential fuzzer**. The retained audit
exercises unbounded recursion and fails loudly; the 715-family campaigns below
have E008 count zero and do not prove stack safety. API/reference default depth
remains 256, bounded native still rejects values above 256, unbounded native
ignores the configured depth, and CLI `--call-depth` is not implemented. ADR 046's
1024 default is the preregistered target, not current behavior.

## B1: memory semantics and removal inventory

Ordinary memory failure replaced deterministic quota equivalence. The dynamic
storage safety guard defaults to **256 MiB**, configurable through Rust APIs and
CLI `--memory-limit`; zero disables it. Clean E013 is resource-inconclusive;
other diagnostics, spans, arithmetic, output, iteration and effects remain exact.
No logical ledger or per-value descriptors were added.

Deleted prospective-copy reservations, virtual stack-singleton charges/releases,
steady-budget capacity restrictions and quota-only replay obligations. Simplified
allocation checks to actual storage/growth. Kept roots/mirrors, child retention,
SSA liveness, collection, checked size arithmetic and allocation-failure handling
because they also protect immutable aliases and memory safety. The full inventory
is in [ADR 046](adr/046.md); renamed/converted tests and raw command receipts are
in the [B1 report](../benchmarks/reports/2026-10-08/ROUND23_B1.md).

The 150–350 net-lines-removed forecast missed. B1 runtime removed 20 net lines,
emitter 9; other API, tests and documentation added lines. Total B1 net change
was **+793**, including tests/docs. No additional quota-only deletion is credited
to B2/B3. The prior fail-fast read change remains ordinary acquisition behavior.

RT-3 is already implemented: both flat and record unique concat assert a rooted
RHS before collection; stack singletons carry a synthetic root. Regression
`concat_growth_roots_heap_and_stack_rhs_across_collection` covers heap, stack and
record-child cases at O0/O2. ADR 043 uses E008, corrected via ADR 046; E010 is not
a new call-depth diagnostic. EXPR_V5's old 32-intrinsic counts are explicitly
historical; current enum count remains **35**.

## B2: natural qualification and cliff measurements

**Natural 19/150 → 19/150; tuned 22/150 → 22/150.** The 20–22 natural forecast
missed. Frozen first-attempt sources were not performance-rewritten; 114 held-out
problems remain untouched. All 29 admitted programs remain correct.

Native O0 natural copied payload fell **1,741.4 GB → 400.057 GB (77.0%)**.
REQUIRED bytes fell 84.8%; AVOIDABLE bytes stayed at 160.056 GB. The general
backward SSA slice permits guarded nontrapping reads across independent updates
and allocations. Caller/parent roots, escaping sequences, unsupported dependencies
and bounded instrumentation remain conservative. G1's interprocedural alias/child
provenance is still unproved; resource freedom alone does not prove uniqueness.

B2 15 rotating warm repetitions on the unchanged largest fixtures:

| Natural program | NIL min / median / max ms | C++ min / median / max ms | NIL / C++ median |
|---|---:|---:|---:|
| Last Stone Weight | 60.362 / 60.944 / 62.929 | 10.569 / 10.815 / 11.089 | 5.64× |
| Balanced Binary Tree | 1496.452 / 1507.334 / 1561.072 | 12.998 / 13.545 / 15.068 | 111.28× |
| Course Schedule | 4252.784 / 4396.318 / 4529.201 | 21.820 / 22.441 / 23.649 | 195.90× |
| Validate BST | 1437.158 / 1460.407 / 1510.473 | 12.418 / 12.972 / 13.674 | 112.58× |
| Spiral Matrix | 26.769 / 27.178 / 28.640 | 7.162 / 7.527 / 8.037 | 3.61× |

The [B2 report](../benchmarks/reports/2026-10-09/ROUND23_B2.md) contains the
before/after census, copy hints, forecast misses and RSS. The
[B3 report](../benchmarks/reports/2026-10-09/ROUND23_B3.md) repeats these timings;
qualification is unchanged. B2 natural peak RSS/C++ MiB: heap 3.73/4.52,
balanced 7.78/11.20, course 18.48/13.31, BST 5.47/8.66, spiral 4.59/6.09.
These are whole-process RSS, not logical charge.

Corrected latest native O0 census (physical payload, not all O2 traffic):

| Scope | Events | Bytes | REQUIRED bytes | AVOIDABLE bytes |
|---|---:|---:|---:|---:|
| Application | 8,027 | 157,594 | 26,310 | 131,284 |
| Natural NeetCode | 3,920,307 | 400,057,040,260 | 240,000,675,731 | 160,056,364,529 |
| Tuned NeetCode | 3,420,177 | 60,208,452 | 3,060,795 | 57,147,657 |
| Fuzz 22001 | 5,288 | 81,120,515 | 81,092,702 | 27,813 |
| Fuzz 22002 | 5,030 | 81,124,146 | 81,092,702 | 31,444 |

The B3 overlay initially missed offset-slice memmove; that measurement bug was
fixed and affected programs rerun. Zero-offset truncation is zero payload moves.
Fuzz denominators changed; only hash-matched common sources support causal
comparisons. Resource-inconclusive fixtures and the largest-3sum trace-cap
exclusion are explicitly listed in the complete reports, not counted as zero
copies. Timing and correctness still include 3sum.

## B3: swap/pop forecast miss and proposed remedy

Bytes/Buffer `!swap`/`!pop` are NIL-source std; no new intrinsic. Swap evaluates
arguments left to right, reads/checks j then i before writes, caches both elements
and returns the updated sequence. Pop returns a shortened sequence; empty input
is E012. Last-use flat slices reuse storage only with one runtime root. Parent and
caller aliases retain copying; record-buffer variants are deferred.

Both cl100k/o200k: `!swap(a,b,c)` **6 tokens** versus exchange **9**;
`!pop(a)` **4** versus explicit slice **10**. One natural heap source has a direct
swap candidate; none has a direct physical-pop candidate. They were not rewritten.

25 rotating warm O2 repetitions, 1,048,576 iterations, exact result checks:

| Operation | Std min / median / max ms | Caller min / median / max ms | Median ratio |
|---|---:|---:|---:|
| Bytes swap | 23.717 / 24.390 / 42.360 | 6.257 / 6.371 / 18.355 | 3.83× |
| Buffer swap | 23.445 / 23.745 / 39.679 | 6.557 / 6.715 / 19.911 | 3.54× |
| Bytes pop | 12.577 / 12.866 / 24.027 | 6.629 / 6.861 / 19.285 | 1.88× |
| Buffer pop | 13.276 / 13.559 / 23.945 | 7.305 / 7.517 / 23.858 | 1.80× |

The within-10% prediction missed all four. Evidence supports a **source-boundary
root-protocol penalty**, not a proven ordinary machine-call cost: LLVM has
alwaysinline typed helpers, but their root traffic survives. ADR 040's HIR
inliner admits only Constant and Buffer/Bytes/Concat instructions, requires every
SSA value to be used exactly once and excludes Index, Replace, Length and Slice.
Both new std bodies fail that eligibility test. Lowered whole-module static
counts: swap std/caller have 2/1 root frames and 19/10 root-store calls; pop 2/1
frames and 15/11 root-store calls. Both select unique storage helpers. Median wall-time excess per iteration is
17.18 ns (Bytes swap), 16.24 ns (Buffer swap), 5.73 ns (Bytes pop), and 5.76 ns
(Buffer pop). These are source-boundary differences, not isolated machine-call
latencies. Static IR counts are not dynamic instruction counts or an exhaustive
attribution of O2 cost.

**Proposal only:** extend general HIR inlining to validated index/replace/slice
bodies, permitting repeated read-only uses while retaining one owned result.
Materialize argument SSA values before substitution, preserve lazy boundaries,
source spans, logical depth and trap order, then rerun the same matrix. The
measured caller form is the achievable comparison, not a guaranteed forecast.
If a general proof cannot meet std parity, evaluate core placement under ADR 035.
No inliner extension or core placement was implemented in this continuation.

Paired B3/B2 controls (25 repetitions): append1MiB 4.468/4.440 ms;
scan **17 MiB** 7.798/7.726 ms; transform16MiB 36.295/36.542 ms. C++ medians are
3.481, 10.396 and 14.229 ms. The scan fixture was historically mislabeled 16 MiB;
its actual size is 17 MiB. No new timing ran alongside validation. Output and
double-transform round trip remain byte-exact. Full samples/RSS are in B3's JSON.

## Resource-inconclusive audit

All **176** archived comparisons were inspected: **88 per seed**, 44 programs
per seed, each at O0/O2. Of these, 172 comparisons have E013 on both paths at the
same span. Four comparisons (`inline_push_quota`, both seeds/optimizations) have
reference E013 and native success. Native output is verified as exactly
33,554,393 zero bytes, byte 1, newline; output-file effects are unchanged. Native
IR uses unique concat; the reference may need a new capacity allocation while
the original remains allocated. That allowed physical-storage difference is the
reason this outcome is resource-inconclusive, not semantic success.

The per-family table is below. Negative-length families also use E013; the
maintainer's all-E013 exception therefore excludes these from semantic parity
claims too. Both-E013 does **not** verify code which would execute after resource
failure, and this audit does not prove such suffixes correct. No archived signal,
malformed diagnostic, sanitizer report or conflicting non-E013 code was found
inside these 88 comparisons; the independent depth audit has real failures and
is reported separately. The fuzzer validates process status before classifying
resource failure. Existing files provide only final native O2 file effects;
per-attempt O0 effects were checked by the original fuzzer but were not separately
persisted, a limitation of this retrospective file audit.

| Family index | Family | Seed 23601 comparisons | Seed 23602 comparisons | Native result at O0/O2 |
|---:|---|---:|---:|---|
| 10 | `base_negative_buffer_length` | 2 | 2 | E013 / E013 |
| 11 | `base_64m_bytes_constructor` | 2 | 2 | E013 / E013 |
| 29 | `base_negative_bytes_length_and_bad_fill` | 2 | 2 | E013 / E013 |
| 92 | `parsebuf_quota_priority` | 2 | 2 | E013 / E013 |
| 120 | `map_quota_boundary` | 2 | 2 | E013 / E013 |
| 128 | `map_constructor_quota` | 2 | 2 | E013 / E013 |
| 129 | `map_byte_constructor_quota` | 2 | 2 | E013 / E013 |
| 130 | `map_put_quota` | 2 | 2 | E013 / E013 |
| 131 | `map_key_result_quota` | 2 | 2 | E013 / E013 |
| 132 | `map_lookup_result_quota` | 2 | 2 | E013 / E013 |
| 162 | `each_ignored_value_quota` | 2 | 2 | E013 / E013 |
| 224 | `record_live_quota` | 2 | 2 | E013 / E013 |
| 235 | `record_map_quota` | 2 | 2 | E013 / E013 |
| 248 | `plugin_live_quota` | 2 | 2 | E013 / E013 |
| 261 | `plugin_equal_bounds_order` | 2 | 2 | E013 / E013 |
| 262 | `plugin_argument_quota_order` | 2 | 2 | E013 / E013 |
| 278 | `collection_negative_length` | 2 | 2 | E013 / E013 |
| 279 | `collection_quota` | 2 | 2 | E013 / E013 |
| 284 | `collection_live_children_quota` | 2 | 2 | E013 / E013 |
| 291 | `collection_zero_quota` | 2 | 2 | E013 / E013 |
| 305 | `layout_nested_zero_negative` | 2 | 2 | E013 / E013 |
| 317 | `bulk_quota_failure` | 2 | 2 | E013 / E013 |
| 325 | `field_append_quota_alias` | 2 | 2 | E013 / E013 |
| 329 | `nested_builder_quota` | 2 | 2 | E013 / E013 |
| 341 | `module_live_quota` | 2 | 2 | E013 / E013 |
| 343 | `module_quota_boundary` | 2 | 2 | E013 / E013 |
| 364 | `host_env_quota` | 2 | 2 | E013 / E013 |
| 381 | `host_directory_quota` | 2 | 2 | E013 / E013 |
| 402 | `hof_quota` | 2 | 2 | E013 / E013 |
| 428 | `std_callback_quota` | 2 | 2 | E013 / E013 |
| 447 | `transfer_quota_partial` | 2 | 2 | E013 / E013 |
| 464 | `reconcile_quota_failure` | 2 | 2 | E013 / E013 |
| 470 | `inline_push_quota` | 2 | 2 | OK / OK; reference E013 |
| 639 | `binding_live_quota` | 2 | 2 | E013 / E013 |
| 655 | `g2_quota_before_read` | 2 | 2 | E013 / E013 |
| 660 | `g2_allocation_barrier` | 2 | 2 | E013 / E013 |
| 702 | `io_quota_real_read` | 2 | 2 | E013 / E013 |
| 704 | `io_quota_before_late_fault` | 2 | 2 | E013 / E013 |
| 705 | `io_buffer_retention` | 2 | 2 | E013 / E013 |
| 706 | `io_record_child` | 2 | 2 | E013 / E013 |
| 707 | `io_record_buffer_child` | 2 | 2 | E013 / E013 |
| 708 | `io_map_and_child` | 2 | 2 | E013 / E013 |
| 709 | `io_selected_lazy` | 2 | 2 | E013 / E013 |
| 710 | `io_callee_retention` | 2 | 2 | E013 / E013 |

## Gates and remaining work

The latest implementation commits are `616dc96` (compiler/std) and `dea2bb6`
(fuzzer); B3's exact gated compiler diff is recorded at `59f2466`. This continuation changes audit tooling and documentation
only. It introduces no partial B4 runtime change.

The existing full-family seeds 23601/23602 each have 715 programs, 1,430 O0/O2
native builds, zero non-E013 divergences **in that generated set**, 88 resource
inconclusives, reference OK525/E01344 and native OK526/E01343 per optimization.
Other per-code counts match: E0098, E01233, E01411, E01512, E01614, E01710,
E01811, E0193, E0206, E02135, E0223. E0080. Host effects are compared. The new
stack witnesses demonstrate that set's coverage gap; the full round is not green.
Application verification remains 2,815 checks, original separately 610; natural
and tuned verification 1,070 each. No frozen source/oracle/golden/baseline changed.
The read-fault gate has 72 exact and 96 resource-inconclusive comparisons.

Offline admission harness tests pass: **13** including six new depth-audit tests.
They verify fail-closed classification, not stack safety. Refreshed CI and
sanitizer-suite command receipts are appended after they finish. The new ASan
stack-overflow witness remains a failure even if the existing sanitizer suite
passes.

Remaining blockers: B4 private-stack/headroom and logical-depth implementation;
COV-1 in default fuzzing; G1 nonlocal alias/child provenance; std boundary root
traffic; record-buffer swap/pop proofs; and the natural cliffs listed above.
No memory ledger, unsafe escape hatch, changed frozen source or raised semantic
quota is used to hide any of them. The Round 23 goal must not be marked complete.

### Refreshed command receipts

| Command | Result |
|---|---|
| `./scripts/ci.sh` | Exit 0; 551 tests; zero warnings |
| `./scripts/ci.sh release` | Exit 0; 551 tests; zero warnings |
| `./scripts/sanitize.sh` | Exit 0; 250 tests; no findings in existing suites |
| `python3 -m unittest discover -s benchmarks/core-memory-study -p 'test_*.py'` | 13 passing offline harness tests |
| `audit_depth.py --out /tmp/nil-depth-admission-new` | Exit 1; 11/36 exact; **25 failures** |
| Targeted large-record bounded O0 ASan/UBSan run | ASan stack-overflow; signal 6; **admission failed** |

See the [admission report](../benchmarks/reports/2026-10-09/ROUND23_ADMISSION.md)
and [JSON receipts](../benchmarks/reports/2026-10-09/ROUND23_ADMISSION.json) for
command timestamps, per-case outcomes, every resource comparison and raw-log
SHA-256 hashes. Raw artifacts remain outside git under
`~/.local/share/nil/round23-memory/depth-admission-committed/` and
`depth-admission-final/`. Existing compiler/fuzzer/std source is byte-unchanged
from `59f2466`; these refreshed gates are not evidence of a B4 fix.
