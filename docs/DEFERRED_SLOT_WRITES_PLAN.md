# Deferred loop-slot writes: implementation plan

The maintainer asked Codex to take over implementation after Kiro reached its
limit. This supersedes the implementer assignment in the local overseer handoff;
no push or merge is authorized. Semantic contract: ADR 055, addenda D–E.

**Status:** direct-slot phase semantics are implemented and validated. The two
original qualification performance cliffs remain unresolved; the forecasts did
not match the programs' actual update shapes.

1. Audit step 4a, the opt-in reference evaluator. Add semantic regressions before
   changing ownership: exact fault spans, chains, aliases, byte validation,
   nested updates and multiple slots sharing a buffer.
2. Implement step 4b: native staging of eligible top-level carried-buffer updates,
   committing after all read-phase expressions. Retain copying for genuine aliases
   and ambiguous ownership. Keep the option off until both paths agree.
3. Implement step 4c: O0/O2 differential tests and fuzz coverage, including
   allocation between staging and commit, host effects, update chains and slot
   permutations. Run focused checks between changes.
4. Implement step 4d only after validation: enable for expr-v5 while preserving
   earlier profiles, document changed diagnostic precedence, verify frozen scan
   programs and inspect copy hints on original-order qualification candidates.
5. Run full debug/release CI, ASan/UBSan, two differential seeds and corpus/depth
   gates. Record actual results and any unresolved limits. Measure qualification
   candidates with interleaved A/B runs; forecasts are not measured results.

After this milestone: request merge approval; then comparator/key sort (count
eligible source shapes first), subtree prefix-slice cliff, worklists/deques and
the next frozen-compiler discovery batch. Generation evaluation stays deferred.

Implementation and validation results follow.

## Implementation

- Audited 4a and reproduced two bugs: an earlier chain write could fault before
  a later slot read, and byte validation was never deferred.
- Replaced partial fault suppression with validated HIR scheduling shared by the
  evaluator and native backend, before native leaf expansion and root analysis.
  Ordinary immutable updates retain all alias, bounds and byte checks.
- Added explicit O0/O2, bounded/unbounded assertions for old-value reads, fault
  spans, effects, aliases, chains and allocation safety, plus 20 fuzz families.
- Enabled the default for Buffer/Bytes slots; retained the eager test hook.
  Earlier profiles have no such dynamic sequences and keep their behavior.
- Updated benchmark helper option literals to match the enabled default. Frozen
  programs, fixtures, baselines and goldens are unchanged.

## Forecast correction

The original-order discovery `redundant-connection` cannot benefit from the
approved direct-slot scope: its slot calls `f`, which conditionally updates its
argument inside the callee. Copy hints still report replacement span `[320,335]`
retained by `[220,235]`. The forecast that this step would remove that copy was
wrong. Interprocedural read/write splitting needs a separate semantic design;
the implementation does not silently widen deferral to calls or lazy updates.

The small `course-schedule` sample reports no replace copies in **either** arm;
it does not exercise the costly path. The largest frozen fixture still reports
two 800,000-byte copies, at `[710,727]` and `[750,760]`, both retained by
`[783,784]`. These update `b.counts` and `b.queue` inside the callee's `Progress`
constructor (line 12), outside the direct carried-input rule. The earlier
small-sample observation was insufficient to establish copy removal.

The eager and deferred `redundant-connection` executables are byte-identical:
SHA-256 `d8f701c6f28f8cf3b9534f9099017fd79727591577db7af60247526a41cfa2cd`.
Its interprocedural copying cliff therefore remains open.

## Interleaved timing

The largest frozen `course-schedule` fixture was measured after CI and sanitizer
jobs finished. Each comparison uses 15 alternating A/B pairs, reversing order
on alternate pairs. Eager, deferred and C++ outputs match the frozen oracle.

| Comparison | First arm median | Deferred NIL median | Ratio |
| --- | --- | --- | --- |
| Eager vs deferred | 4,300.82 ms | 4,252.92 ms | 0.9889 new/old |
| C++ vs deferred | 22.31 ms | 4,264.86 ms | 191.17 NIL/C++ |

There is no material speed improvement, and this program still misses the 3×
qualification gate. The direct-slot reproducer avoids its required old-state
copy in both slot orders; that result does not remove callee/record-field copies.
The forecast of 3–6× C++ for this program was also wrong. No new qualifier is
claimed from this work.

Raw reports: `/tmp/nil-slot-course-ab.json` and
`/tmp/nil-slot-course-cpp-ab.json`. The helper source is
`/tmp/nil-slot-build.rs`; its only arm difference is `Options.defer_slot_writes`.
Source hashes, executable hashes and output validation accompany the final
receipt. Timing commands:

```sh
python3 scripts/ab_time.py /tmp/nil-slot-course-eager \
  /tmp/nil-slot-course-deferred --pairs 15 \
  --task /tmp/nil-slot-course-schedule-largest.input \
  --task /tmp/nil-slot-course-largest.output --output /tmp/nil-slot-course-ab.json
python3 scripts/ab_time.py /tmp/nil-slot-course-cpp \
  /tmp/nil-slot-course-deferred --pairs 15 \
  --task /tmp/nil-slot-course-schedule-largest.input \
  --task /tmp/nil-slot-course-largest.output --output /tmp/nil-slot-course-cpp-ab.json
```

The original R37 copying goals remain open. Round 38 explicitly parks callee and
record-field optimization; the next authorized work is comparator/key sort and
the general last-use prefix-slice rule. No further optimization chain follows
from these unfavorable measurements.

## Validation

Focused reference tests: 7 passed. Native deferred-write tests: 6 passed, with
O0/O2 and bounded/unbounded checks inside each. Read-replay suite: 27 passed.
Opt-in slot fuzz seed 96431: 20 programs / 40 native builds, zero divergences,
zero inconclusives. Final debug CI includes formatting and strict Clippy.

Completed broader checks:

| Check | Result |
| --- | --- |
| Debug CI | 682 tests passed; formatting and strict Clippy clean |
| Release CI | 682 tests passed |
| ASan/UBSan | 326 tests passed; zero findings |
| Differential seed 96431 | 903 programs, 1,916 native builds; zero non-resource divergences |
| Differential seed 107309 | 903 programs, 1,916 native builds; zero non-resource divergences |
| Application corpus | 2,815 checks passed; original 610 retained |
| Natural cohort | 1,070/1,070 checks passed |
| Tuned cohort | 1,070/1,070 checks passed |
| Depth audit | 36/36 comparisons passed |
| Discovery subtree/redundant-connection | 70/70 checks passed |
| Frozen plus-one density kernel | 7 reference + 7 O0 + 7 O2 checks passed |

Each full fuzz seed retains 36 E008 resource comparisons as inconclusive; there
are zero rule-3 failures.

Final combined receipt: `/tmp/nil-slot-final-receipt.json`. It records the
passing reruns, retained original gate receipt, source and executable hashes,
timing reports and unresolved copy sites. All compiler/test source hashes match
the validated snapshot. This closes implementation and correctness validation
of the scoped phase rule; it does **not** close the original performance goals.

Full gate command:

```sh
NIL_BENCH_HOME=/tmp/nil-slot-bench-home ./scripts/gates.sh full \
  --seeds 96431,107309 --out /tmp/nil-slot-full
```

Validation ran on an uncommitted working tree. Round 38 Step 1 records that
already-validated work in separate local commits. The gate records dirty state
and binary hashes; `/tmp/nil-slot-full/source-manifest.json` records source hashes.
The main checkout is used because Codex now owns implementation and Kiro is
inactive; the old handoff's separate-worktree rule prevented concurrent edits.

The first full gate's corpus and OS-entropy tests were blocked by macOS refusing
nested `sandbox-exec` with `sandbox_apply: Operation not permitted`. The corpus
and sanitizer checks and both CI modes passed after approved execution outside
the Codex sandbox. The original receipt records the failed nested-sandbox runs.
Raw rerun logs live at
`/tmp/nil-slot-{corpus,sanitize-approved,ci-approved,release-approved}.log`.
The gate wrapper now rebuilds both `core_check` and `application_check` before
freezing artifacts; previously it could retain a stale application helper.

Rerun commands:

```sh
env CARGO_TARGET_DIR=target/gates-debug ./scripts/ci.sh
env CARGO_TARGET_DIR=target/gates-release ./scripts/ci.sh release
./scripts/sanitize.sh
env NIL_PREBUILT=1 NIL_BIN=/tmp/nil-slot-corpus-bin \
  benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py \
  --output /tmp/nil-slot-corpus.json \
  --original-output /tmp/nil-slot-corpus-original.json --jobs 6
```

Changed existing expectations: `read_replay` now expects no replace copy in both
slot orders; its original copying witness explicitly opts into eager behavior.
`application` now expects a unique write after scalar readers in bounded and
unbounded IR. No golden diagnostic fixture has changed; new tests specify moved
bounds spans and read-fault precedence over byte-value faults.
