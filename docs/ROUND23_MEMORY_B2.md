# Round 23: dependency-sliced replay (B2)

This is the completed, gated B2 checkpoint. Round 23 remains incomplete. G1 interprocedural provenance, std swap/pop and native depth/stack
safety are not part of this implementation. No natural source was tuned.

## Proof and implementation

The B2 preregistration was committed in `2afaa70` before compiler `9129976`
and fuzz `4848a78`. The committed code matches the gated working-tree patch
byte-for-byte; receipts retain its SHA-256.
The native emitter uses a backward SSA slice of every later local use of the
original Buffer/Bytes value, with semantic liveness's operand/capture traversal.
Only needed i64/bool observations and their scalar dependencies are probed.
Independent allocations, updates and effects remain at their original sites.

The first replacement's bounds/byte checks precede every probe. Probes never
raise a diagnostic: their validity guards choose cached scalar observations or
ordinary execution. Only a complete successful guard conditionally clears the
original local root. The slot remains reserved through its original last use;
the false path retains the pointer. Caller aliases, returned aliases and record/
map children keep their roots and runtime uniqueness checks. No new provenance
assumption permits clearing a parent or a caller's borrowed root.

Escaping sequences, dependencies on a new sequence result, unsupported selected
operations, and finite replay-expansion limits remain conservative. Bounded
instruction/call-budget execution retains its existing behavior. G1 requires a
separate returned-alias/child-provenance fixed-point proof and is deferred.

## Validation and measurement

Raw receipts belong outside git under
`~/.local/share/nil/round23-memory/ordinary-b2/`. Final gates:

| Command | Result |
|---|---|
| `./scripts/ci.sh` | 545 tests; zero warnings |
| `./scripts/ci.sh release` | 545 tests; zero warnings |
| `./scripts/sanitize.sh` | 246 tests; no findings |
| `./scripts/fuzz-v5.sh --seed 23501` | 701 families; 1,402 native builds; zero non-E013 divergences; 88 inconclusive comparisons |
| `./scripts/fuzz-v5.sh --seed 23502` | Same counts; independent seed |
| `audit_read_order.py --repeats 3 --require-parity` | 14 families; 72 exact and 96 resource-inconclusive comparisons; zero diagnostic divergences |
| Application `verify.py` | 2,815 checks; zero inconclusive |
| Application `verify.py --group original` | Exactly 610 checks; zero inconclusive |
| NeetCode `verify.py --verified-only` | 1,070 tuned checks; zero inconclusive |
| NeetCode `verify.py --candidate-dir .../natural` | 1,070 natural checks; zero inconclusive |

Timing/RSS and all five copy scopes are complete. The application and tuned
scopes are exactly unchanged; 674 hash-matched bounded fuzz cases per seed
also have unchanged totals. Thirteen shifted acquisition-fault sources are
unmatched and fourteen new cases change the denominator. Each census excludes
36 resource-inconclusive fixtures; both cohorts retain the pre-existing largest
3sum native trace cap. See the [complete report](../benchmarks/reports/2026-10-09/ROUND23_B2.md)
and its JSON for per-code/family counts, all timings/RSS, copy data and receipts.
Fresh qualification remains natural 19/150 and tuned 22/150. Natural heap
improves 25,180.46 → 60.94 ms but remains 5.64× C++; the other four cliffs remain
unqualified. Natural native O0 payload falls 1,741.4 → 400.1 GB; REQUIRED bytes
fall 84.8%, AVOIDABLE bytes are unchanged. No qualification gain is claimed.
Forecasts in ADR 046 are not measurements. Each seed has 693 distinct source
hashes; the pair has 785 distinct hashes with 601 shared. Fixed structural
witnesses and varied constants are not unrestricted grammar diversity. E008 is
not exercised by these campaigns; unbounded stack safety remains a B4 blocker.

Initial test corrections: copy-hint witnesses that now optimize were replaced
with genuinely copied alias or invalid-read witnesses. One new fuzz fixture
incorrectly declared a record after the generated entry signature; the existing
all-family typing test caught it, and it now uses the harness's existing Cell.
The corrected final gates found zero non-E013 divergences and no sanitizer
findings. E008/unbounded stack safety remains an explicit B4 defect, not a passed
guarantee.
