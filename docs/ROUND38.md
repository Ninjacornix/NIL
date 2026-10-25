# Round 38 — deferred slots, comparator sort and prefix slice

**Unfavorable first:** deferred slot writes did not fix either original copying
cliff. Natural `course-schedule` measured 4,300.82 ms eager versus 4,252.92 ms
deferred; a separate 15-pair comparison measured 191.17× C++. Both 800,000-byte
copies remain inside the callee's `Progress` constructor. Original-order
`redundant-connection` produces byte-identical eager/deferred executables because
its update is inside a callee. No new qualifier is claimed. These are inherited
2026-10-24 measurements; the overseer owns Round 38 timing. Callee/record-field
optimization is parked under the maintainer's language-first rule.

| Step | Status | Delivered or required |
| --- | --- | --- |
| 1: deferred slot writes | Shipped locally | ADR 055 E, shared validated HIR scheduling, evaluator/native integration, tests, fuzz, docs and benchmark helper defaults/pointer |
| 2: comparator/key sort | Not shipped | Await audit completion; count exact shapes in every cohort, then ADR 056 forecasts before code |
| 3: prefix slice | Not shipped | Await audit completion; semantic tests before general unique last-use truncation and append reuse |

Step 1 applies only to direct top-level carried Buffer/Bytes result chains whose
intermediates have no other consumers. Nested updates, fixed arrays, record
buffers and multiple result chains on one carried input remain eager. Existing
alias checks retain copying. Read-phase faults/effects precede staged bounds and
byte checks; fault spans can move. The two repaired 4a bugs were an earlier chain
write fault escaping deferral and byte validation staying eager. ADR 055 E is
explicitly marked as written during implementation. No compiler behavior was
changed while packaging these commits.

Last validation: **682 debug / 682 release**, formatting and strict Clippy clean;
**326 sanitizer tests**, zero findings; two **903-program** fuzz seeds
(`96431`, `107309`), zero non-resource divergences/rule-3 failures, 36 E008
inconclusive comparisons per seed. Corpus **2,815 / original 610**; natural and
tuned cohorts **1,070 each**; depth **36/36**. The first gate was blocked by
nested macOS sandboxing; approved reruns passed. Provenance and logs:
`/tmp/nil-slot-final-receipt.json`, with details in
[the implementation record](DEFERRED_SLOT_WRITES_PLAN.md).

After the maintainer reports the audit complete, Step 2 starts by counting actual
key extraction, record/row, multi-key and descending shapes per program across
all cohorts. TOKEN_GAP is an upper bound, not a ranking. Measure at least three
spellings with cl100k/o200k, prefer reuse of `!sort` and static `&f`, and record
stable deterministic semantics and forecasts in ADR 056 before code. Rewrite
only counted programs into a new cohort; report before/after/Python tokens and
measured xC++. Step 3 must prove last-use uniqueness before truncating a prefix
in place, including alias and later-read negatives; inspect the original subtree
program with `nil --profile expr-v5 --explain-copies run ...`.

Overseer tasks in `~/nil-audit`: pin both local repository heads; audit commit
order and unchanged frozen inputs; run the closing gate with independent seeds.
Own all interleaved timing: Step 1 controls and unchanged cliffs; Step 2 counted
sort kernels versus equivalent `std::sort`, including duplicate keys, multi-key
and descending cases; Step 3 original subtree before/after/C++ plus append/scan/
transform controls. No heavy jobs after Step 1 until the maintainer releases
the audit checkpoint. All commits stay local; nothing is pushed.
