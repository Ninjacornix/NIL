# expr-v4: typed arrays, tokens and runtime

Historical lowering report. [The storage follow-up](EXPR_V4_STORAGE.md) supersedes
the bulk-update performance limitation below while retaining identical NIL source.

These local macOS/Apple Silicon experiments cover 25 fixed-array/bool programs,
not general-purpose workloads or model-generated candidates. [Raw primary data](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v4-typed.json),
[fresh-local mutation comparison](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v4-local.json), [validation/payload data](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v4-validation.json),
and the [initial lowering baseline](https://github.com/Ninjacornix/NIL/blob/a8772dcd8b5ccc14341c8ed98bbd0c652cc1ee84/benchmarks/paired/results/2026-09-30/expr-v4-typed-baseline.json) preserve settings,
source hashes, tokenizer revisions, oracle checks and samples.

## Selected semantics

Keep v3 wrapping i64 and add explicit bool/array function signatures plus immutable
fixed-length i64 arrays (0..256). Checked reads, value replacement and static length
answer sum/dot/max/search/reverse/prefix programs without exposing pointer ownership
or allocation. Array values cross typed calls, branches and simultaneous loop state.
The LLVM bridge separates native tooling slots from aggregate calling conventions.
See [ADR 014](../../../docs/adr/014.md) and the [spec](../../../docs/language/EXPR_V4.md).
These are the selected M3 experiment, not proven universally optimal semantics.

## Token results

Actual source tokens, excluding shared timing/entry adapters:

| Corpus / encoding | cl100k_base | pinned Qwen Coder |
|---|---:|---:|
| NIL, 25 programs | 851 | 906 |
| Python, explicit-copy transforms | 1292 | 1292 |
| Python, fresh-local transforms | 1068 | 1068 |
| C++, checked read/value-replacement helpers | 4551 | 4888 |

C++ totals include required headers/helpers. The 24 array function bodies alone
use 1756/1955 tokens, versus NIL's 840/895 for those same programs. This separates
context from algorithm size; adding helper context to C++ is not itself proof of
semantic compression. Fresh-local C++ baselines have their own complete-source
counts in the local report. Source hashes/characters/bytes accompany every count.

Sum uses 23 cl100k tokens at every measured length and 23..25 Qwen tokens. V3
unrolled sums use 12/12 tokens at length 8, 45/46 at 32, 237/239 at 128 and 496/498
at 256. Arrays therefore do not make every small program shorter. Signature-only
screens favor numeric lengths (23/23) over tagged lengths (24/24), bracket lengths
(26/26), and explicit full types (39/44). Alternative grammars are hypothetical;
this screen measures tokenizer behavior, not model acceptance or repair cost.

## Native/runtime results

Median ns/call from the primary run (NIL O2, C++ O2 -fwrapv):

| Program | N | NIL | C++ value baseline | Python value baseline | NIL / C++ |
|---|---:|---:|---:|---:|---:|
| sum | 8 | 0.95 | 0.95 | 198.27 | 1.00 |
| dot | 8 | 2.30 | 2.30 | 302.92 | 1.00 |
| max | 8 | 1.10 | 1.05 | 200.38 | 1.05 |
| search | 8 | 3.25 | 3.45 | 167.48 | 0.94 |
| reverse | 8 | 14.45 | 1.75 | 692.38 | 8.26 |
| prefix | 8 | 14.65 | 2.05 | 781.73 | 7.15 |
| sum | 32 | 1.95 | 2.00 | 607.04 | 0.97 |
| dot | 32 | 13.60 | 11.40 | 960.90 | 1.19 |
| max | 32 | 3.30 | 3.30 | 582.48 | 1.00 |
| search | 32 | 20.10 | 7.05 | 224.85 | 2.85 |
| reverse | 32 | 197.40 | 214.40 | 3211.69 | 0.92 |
| prefix | 32 | 200.90 | 234.60 | 3952.94 | 0.86 |
| sum | 128 | 39.90 | 7.40 | 2554.94 | 5.39 |
| dot | 128 | 145.40 | 60.90 | 3861.33 | 2.39 |
| max | 128 | 58.20 | 29.30 | 2106.77 | 1.99 |
| search | 128 | 49.65 | 20.70 | 321.23 | 2.40 |
| reverse | 128 | 5846.00 | 5186.95 | 32785.75 | 1.13 |
| prefix | 128 | 5395.45 | 4955.20 | 34985.42 | 1.09 |
| sum | 256 | 106.55 | 36.80 | 4384.12 | 2.90 |
| dot | 256 | 270.40 | 120.95 | 7222.15 | 2.24 |
| max | 256 | 132.00 | 62.55 | 3996.58 | 2.11 |
| search | 256 | 118.25 | 39.10 | 331.88 | 3.02 |
| reverse | 256 | 20586.00 | 19902.10 | 99410.06 | 1.03 |
| prefix | 256 | 20888.60 | 20034.55 | 104586.94 | 1.04 |
| predicate | 1 | 0.95 | 0.95 | 59.35 | 1.00 |

Read-only loops initially stored entire input aggregates into scratch space every
iteration. Optimized LLVM IR retained those stores; a regression test reproduced
that behavior. The fix snapshots immutable parameters once and reuses storage for
identity-carried loop state, while replacement always has separate writable storage.
The old report is retained. Before/after runs have different batch counts and host
conditions; their absolute times are not a controlled speedup percentage.

The more important transform comparison uses fresh, unaliased local result arrays.
In-place updates are externally equivalent to immutable return-value construction:

| Program | N | NIL ns | C++ local ns | Python local ns | NIL / C++ local |
|---|---:|---:|---:|---:|---:|
| reverse | 8 | 15.10 | 1.85 | 323.79 | 8.16 |
| prefix | 8 | 15.55 | 2.25 | 444.54 | 6.91 |
| reverse | 32 | 201.50 | 8.55 | 913.12 | 23.57 |
| prefix | 32 | 202.55 | 9.70 | 1347.92 | 20.88 |
| reverse | 128 | 5378.45 | 62.85 | 3264.21 | 85.58 |
| prefix | 128 | 5255.80 | 90.65 | 5015.21 | 57.98 |
| reverse | 256 | 20623.40 | 140.60 | 6722.10 | 146.68 |
| prefix | 256 | 20849.40 | 198.20 | 9955.33 | 105.19 |

At 256 elements, NIL reverse/prefix are about 147×/105× slower than local C++, and
3.07×/2.09× slower than local Python. Updated aggregate loop state still causes
O(N²) copying in the current lowering. Immutable semantics permit eliminating
copies when aliases and simultaneous updates remain correct; they do not require
this implementation cost. Keep the value contract, but do not claim fast bulk
updates until ownership-aware storage lowering has independent alias/trap tests.
That optimization and a future slice/reference contract are separate decisions.

## Compiler and validation cost

Primary O2 frontend samples range 14..39 microseconds; total backend build samples
range 93..387 milliseconds and binaries 34,408..51,048 bytes. These are single
build-stage observations, not latency distributions. Repeated in-process frontend
medians range 1.042..10.458 microseconds; independent HIR validation medians range
0.208..1.666 microseconds (101 samples each, file I/O/cloning/destruction excluded).
Flat input payloads range 8..4096 bytes. Payload size is not source length or host
object size. Raw data separates IR lowering, LLVM codegen, runtime compile and link.

## Method and limits

All 79 primary fixtures pass Python, C++, NIL O0 and NIL O2 (316 checks); local
transform variants also pass the independent fixtures without changing inputs.
The native implementations use the same generated C timing driver and flat ABI,
separate translation units and no LTO. Seven timing repetitions follow 1000 warmups;
primary O2/C++ batches use 20,000 calls, O0/Python 2000. Local O2/C++ use 20,000,
Python 2000. Parsing/startup/compilation are excluded; marshaling and output checks
are included. Results depend on host scheduling/frequency; tiny timings are not
universal performance rankings.

Inputs/intermediates fit i64; sorted-search arrays are unique. Python's general
integer/division semantics and negative indexing are not claimed equivalent outside
these domains. C++ bounds helpers abort instead of producing NIL's diagnostic.
NIL-specific wrapping/bounds/lazy/alias cases are covered in compiler/native tests.
No generation tokens, model parse/type success rates, repair cost or TCR were
measured. A syntax that saves raw tokens can still cost more tokens to correct.

## Reproduce

```sh
python3 benchmarks/paired/typed/generate.py
benchmarks/paired/.venv/bin/python benchmarks/paired/typed.py \
  --output /tmp/v4.json --iterations 20000 --o0-iterations 2000 \
  --python-iterations 2000 --repeats 7
benchmarks/paired/.venv/bin/python benchmarks/paired/typed_local.py \
  --output /tmp/v4-local.json --iterations 20000 --python-iterations 2000 --repeats 7
benchmarks/paired/.venv/bin/python benchmarks/paired/typed_check.py --output /tmp/v4-validation.json
```

Use the existing pinned Python environment from [benchmark setup](../../paired/README.md).
The next type/representation decision needs model trajectories or a new acceptance
program; runtime optimization should first address aggregate loop-state storage.
