# Round 23: flat-sequence std updates (B3)

**The speed forecast missed.** Std swap takes 3.54–3.83× the equivalent
checked caller code; pop takes 1.80–1.88×. Both have verified in-place paths,
but the forecast was within 10%. Natural qualification remains 19/150,
tuned 22/150. B4 remains open: default unbounded recursion still crashes
instead of reporting E008. **Round 23 is not complete. Nothing was pushed.**

## Implementation and semantics

ADR 046 preregistered B3 before compiler commit `616dc96` and fuzz commit
`dea2bb6`. `!swap(seq,i,j)` and `!pop(seq)` are validated, compiler-versioned
NIL-source std functions for Buffer/Bytes. Intrinsic count remains **35**.
No earlier profile, default, frozen corpus source or held-out problem changed.

Swap evaluates arguments left to right, then reads/checks j and i before
writing. It caches both original elements. Pop returns the shortened sequence;
empty input reports E012. Diagnostics retain normal std-source provenance.
Record-buffer variants are deferred pending a child-edge truncation proof.

The existing last-use analysis selects a generic flat unique slice path.
Bounds are checked and roots synchronized before mutation. Exactly one runtime
root permits overlapping-safe offset movement and a length update, retaining
capacity. Aliases and parent-held children keep copying. This does not add views
or weaken ownership proofs. Tests cover aliases, map/record retention,
zero-slot parents, loops, lazy arms, trap order and host effects at O0/O2 in
bounded and unbounded builds.

## Measurement

tiktoken 0.12.0, cl100k/o200k: swap **6** tokens versus functional exchange
**9**; pop **4** versus slice **10**. There is one direct natural heap-swap
candidate and no direct physical-pop candidate. Natural sources remain frozen.

| Control | Std median ms | Caller median ms | Ratio |
|---|---:|---:|---:|
| Bytes swap | 24.390 | 6.371 | 3.83× |
| Buffer swap | 23.745 | 6.715 | 3.54× |
| Bytes pop | 12.866 | 6.861 | 1.88× |
| Buffer pop | 13.559 | 7.517 | 1.80× |

These are 1,048,576-iteration O2 controls with 25 rotating warm samples,
exact result checks and all samples retained. They measure the std boundary,
not C++ parity. Source-callee/root machinery remains; the restricted HIR leaf
inliner does not admit these bodies. Wall time alone does not attribute every
remaining instruction. Lowered IR has two std root frames versus one
caller frame; swap has 19 versus 10 root-store calls, pop 15 versus 11.
Both forms select the unique helpers. These are static whole-module counts,
not measured dynamic O2 costs.

Paired 25-repeat controls against a fresh B2 build: append 4.440 → 4.468 ms,
historical **17 MiB** scan 7.726 → 7.798 ms, transform16MiB 36.542 → 36.295 ms.
No material regression is demonstrated. Output and double-transform round trip
remain byte-exact. Full min/median/max, C++ comparisons, RSS, frozen-cohort
timings and copy census are in the [complete report](../benchmarks/reports/2026-10-09/ROUND23_B3.md)
and [machine-readable results](../benchmarks/reports/2026-10-09/ROUND23_B3.json).

## Gates and findings

Raw receipts are outside git at
`~/.local/share/nil/round23-memory/ordinary-b3/`.

| Command | Result |
|---|---|
| `./scripts/ci.sh` | 551 tests; zero warnings |
| `./scripts/ci.sh release` | 551 tests; zero warnings |
| `./scripts/sanitize.sh` | 250 tests; no findings |
| `./scripts/fuzz-v5.sh --seed 23601` | 715 indexed cases; 1,430 native builds; zero non-E013 divergences; 88 inconclusive comparisons |
| `./scripts/fuzz-v5.sh --seed 23602` | Same counts; independent seed |
| `audit_read_order.py --repeats 3 --require-parity` | 14 families; 72 exact / 96 resource-inconclusive comparisons; zero diagnostic divergences |
| Application `verify.py` | 2,815 checks; zero inconclusive |
| Application `verify.py --group original` | Exactly 610 checks; zero inconclusive |
| NeetCode natural/tuned `verify.py` | 1,070 checks each; zero inconclusive |

Each fuzz campaign compares effects and code/span at O0/O2. Fourteen new
families cover swap/pop/slice aliases, parent retention, bounds, lazy arms,
loops and effects. Per-code and family counts are in the complete report.
E008 is not exercised; these campaigns prove no unbounded stack safety.

The copy-census audit caught a missing hook for the new in-place offset-slice
memmove. The corrected overlay counts those physical moves and excludes
zero-offset truncation. Affected programs were rerun; selection and merge
receipts require matching source hashes and unchanged reference counts.
Eighteen copy-harness tests pass, including instrumentation and fail-closed
selection regressions. Preliminary undercounted results are not the finding.

No final non-E013 divergence or sanitizer finding was discovered. Test preflight
corrections concerned existing diagnostic wording, an invalid benchmark arity
spelling and a probe's entry selection; none was a compiler bug.

## Remaining admission boundary

G1 interprocedural alias/child provenance remains unproved. Record-buffer
truncation and std call overhead remain open. B4 has no partial implementation.
The reported 200,000-deep recursion was reproduced: unbounded O0/O2 both signal
11; bounded O0/O2 report E008 at `40..46`.

Clang can report actual machine-frame sizes, including after LTO, but this is
not a complete stack-safety proof. Inlined and replayed source calls also need
logical depth guards. A private stack, pre-call headroom checks and deep/large-
frame sanitizer evidence are required before B4 can ship. These outstanding
requirements are not waived by the passing B3 gates.
