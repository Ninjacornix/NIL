# V5 performance debts: diagnosis before builder changes

Starting revision: 21db415. Raw outputs initially under
`/tmp/nil-performance-debts/`; final report will preserve receipts in benchmarks.

## Record-field append

Unchanged LLVM IR, O2/LTO, 25 alternating paired whole-process samples:

| Runtime-only reference | Before ms | Current ms |
|---|---:|---:|
| pre-hardening collections d72a23d | 340.153 | 392.296 |
| pre-collections 1689ffe | 392.117 | 404.896 |

`debt_diagnosis.py --repeats 9` instruments the same IR and three C runtimes.
At 160k all three execute 320,003 malloc/free pairs, 480,000 memcpy calls,
12,800,240,000 copied bytes, 799,998 arena-list visits, **zero child-row visits**.
At 10k/40k copying is 50,015,000/800,060,000 bytes: quadratic. Median copy
component costs at 160k are 313.106/291.008/308.167 ms respectively; allocation
13.451/13.174/13.187 ms and free 15.321/16.111/15.062 ms. Instrumentation
perturbs inlining and timing, so these are component attribution, not a substitute
for uninstrumented paired medians. Native disassembly shows three memcpy sites
in each version. The comparison depends on the reference runtime's generated
copy path, rather than new child traversal or changed algorithmic work. Exact
microarchitectural attribution of its residual throughput difference is not yet
established; it must not be claimed as root bookkeeping or unavoidable overhead.

The structural cause is identifiable: projecting `a.data` adds a root while the
old aggregate still owns a field root. Runtime uniqueness correctly rejects
in-place concat. Geometric capacity alone cannot help if every append copies.

## Proof to attempt

For a direct dynamic field projected into concat, prove that (1) the old record's
last consumer is the functional update of that same field, (2) no intervening
consumer reads that old field or passes/escapes the record, (3) the concat result
is consumed only by that update, (4) the update immediately follows concat.
Transfer the old field's root to the projected operand, leaving all other fields
rooted. Alias roots in other fields, values, callers and arena parents remain and
continue to defeat runtime uniqueness. The old aggregate's stale pointer is never
dereferenced again: its next consumer overwrites that field. There is no allocation
between concat and overwrite, so the same conservative pre-concat quota reservation
suffices without exposing the temporary physical reuse through E013. Reject all
unproved chains; do not introduce user-visible ownership or reduce reservation.

For nested builders, attempt keeping a single-use concat state root across the
backedge, as with existing replacement chains. Normal last-use root updates still
apply to temporaries; allocation-free condition execution cannot observe retention.
Only a final direct concat from that state is admitted initially. Copy fallback
continues to handle live aliases. Collection-list traversal for freshly allocated
children remains a separate potential quadratic cost even if edge re-retention
is eliminated; measure it rather than promising linearity.

## Controlled cause and correction

A frozen 21db415 compiler with identical field-append IR was compiled twice,
changing ONLY the `NIL_RECORD_BUFFERS` macro. At 160k the ordinary specialized
path costs 419.038 ms, the generic child-aware path 340.558 ms (25 alternating
samples). The binaries have 0 versus 53 out-of-line retain/drop sites. Both
perform the same quadratic copying and no child-row traversal. Thus the prior
comparison-dependent loss is reproducible from root specialization's compilation
context alone, not extra rows or an algorithm change. The observed cause is
inlining/code-layout sensitivity in the forced-copy workload; attributing exact
CPU/cache cycles beyond this controlled intervention would be speculation.
Dead-field root transfer removes the forced copies instead of selecting the slow
ordinary runtime for every program. It preserves all genuinely observable aliases.

The first deferred-sweep policy produced a separate real regression: keyed repack
64k rose from historical 74.495 to 155.793 ms; retained nested field updates also
slowed. Dead copied payloads accumulated in the physical arena. A live-relative
garbage budget now triggers earlier reclamation, retaining geometric sweep spacing
for growing graphs while preventing those large dead working sets. Early all-green
validation/measurements are superseded by a complete campaign after this correction.
Final command receipts and final medians will be linked in the benchmark report.


## Final acceptance after the flat-arena correction

Applying deferred collection to flat programs slowed ordinary append in a paired
control (23.85 → 25.27 ms). The existing validated whole-program record-buffer scan
now keeps eager collection for flat arenas and bounded deferred collection only
where child edges exist. A 25-repeat prototype recovered 24.30 → 22.33 ms; final
controls, not that prototype, are the delivery numbers below.

Final gates: `./scripts/ci.sh` and `./scripts/ci.sh release` each pass **368 tests**,
zero warnings; `./scripts/sanitize.sh` passes **142 tests**, all six suites.
Seeds 1729, 8675309 and 5130572 each compare 332 programs at O0/O2: **996 programs,
1,992 builds, zero divergences**. Each exercises all twelve new bulk-comparison,
six field-transfer and six nested-builder families; code/family counts are in the
[JSON receipt](../../benchmarks/reports/2026-10-05/V5_DEBTS_FUZZ.json).
Corpus verification passes **2,775 checks**, original **610** separately, with
all frozen source hashes unchanged. No correctness/sanitizer failure was found
this round; performance controls exposed both collection-policy regressions above,
which were corrected before this final campaign.

Final O2 equality16MiB **7.22 ms**, field160k **7.57 ms**;
append1MiB **22.85 ms**, scan16MiB **7.60 ms**, transform16MiB
**36.67 ms**. The [complete report](../../benchmarks/reports/2026-10-05/V5_PERFORMANCE_DEBTS.md)
contains before/after builder curves, all historical control comparisons, remaining
live-parent/alias copying limits and exact [command output](../../benchmarks/reports/2026-10-05/V5_DEBTS_RECEIPTS.txt).
[IR excerpts](../../benchmarks/reports/2026-10-05/V5_DEBTS_IR.txt) show the general
bulk lowering, dead-field transfer and an alias counterexample retaining copying.
No new surface, task, oracle, golden, baseline or density experiment was added.
All commits are local; nothing was pushed.
