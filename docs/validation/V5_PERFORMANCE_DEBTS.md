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
