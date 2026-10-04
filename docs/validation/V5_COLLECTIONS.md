# Expr-v5 structured collection validation

[ADR 030](../adr/030.md) was committed as 6eb11b6, with capability amendment
2b67d31, before implementation. [The specification](../language/EXPR_V5.md#dynamic-record-buffers-adr-030)
covers representation, layout, lifetime/aliasing, quota, failure ordering and deferrals.

## Command output

[Receipts](../../benchmarks/reports/2026-10-04/V5_COLLECTION_RECEIPTS.txt) retain
Cargo result lines, seeded summaries and corpus/density totals. All final commands
completed successfully against stable code.

| Command | Result |
|---|---|
| `./scripts/ci.sh` | 353 passed, zero warnings |
| `./scripts/ci.sh release` | 353 passed, zero warnings |
| `./scripts/sanitize.sh` | 131 passed, all six suites; ASan/UBSan/float-cast checks clean |
| `./scripts/fuzz-v5.sh --seed 1729 --cases 288 ...` | 288 programs, 576 O0/O2 builds, zero divergences |
| Same with 8675309 and 5130572 | 288/576/zero each |
| Corpus `verify.py` | 79 tasks, 66 NIL references, 13 unsupported, 2,775 passing checks |
| `verify.py --group original` | Original 610 passing checks |
| `algorithms/check_random.py --seed 30030 --cases 30 ...` | 600 supplemental checks |
| `collections/probes.py` | 12 storage-layout checks; not complete task solutions |
| Corpus unittest discovery | 23 pass, frozen source/oracle/baseline locks preserved |
| `density.py` with fresh verification | Four tokenizers; 66 JSONL positives; Qwen cross-check 200/zero mismatches |

Sanitizers instrument both emitted IR and C runtime at O0/O2. macOS leak detection
is unavailable; no leak claim. The strengthened snapshot test modifies the iterated
collection while subsequent original elements remain visible. Tests also cover
empty/bounds/quota, retained/reclaimed children, repeated dynamic aliases,
functional growth/replacement/slice, calls, packed scalar bits and deferred sorting.

Per fuzz seed: **OK207, E0092, E01217, E01320, E0145, E0154, E01610, E0173,
E0185, E0192, E0205, E0215, E0223**. Aggregate is triple. All 24 new collection
families execute once per seed; JSON includes every call-family and mutation count.
This is bounded template coverage, not exhaustive arbitrary recursive/nested types.
Some source substitutions do not apply to a template; mutation counts do not imply
that every mutation was rejected. No final diagnostic-code/output divergence occurred.

## Load-bearing proof boundaries

Validated earlier-type IDs and depth ≤32 make the dynamic ownership graph acyclic.
Word-slot widths/offsets are checked against nominal definitions (≤4096 slots).
Record buffers charge 40+(capacity+1)*W plus separately deduplicated child allocations.
A rooted outer value retains child-edge multiplicity, including repeated handles.
A child extracted while a parent remains live cannot become falsely unique.
Replacement retains new edges before releasing old; no allocation/collection occurs
inside root-transfer gaps. Copies share immutable children and add their own outer root.

The existing last-use proof admits record-buffer replacement/concat only with
runtime uniqueness checks. [IR output](../../benchmarks/reports/2026-10-04/V5_COLLECTION_STORAGE_IR.txt)
shows `nil_record_set(... i1 true ...)` for the single-use scalar loop and
`i1 false` for a live alias; both are asserted by a regression test. New record
element access uses checked C copying, not the earlier byte-range unchecked path.
Record sorting, generic nested maps and sequence-bearing record map values remain
deferred rather than inventing ordering or packing unsafe handles into keyed bytes.

## Algorithms, density and performance

Four MIT-provenance programs demonstrate indexed linked-list traversal, BST insertion/
in-order walk, nested-adjacency BFS and interval merge. Independent random oracles
check 30 inputs each on reference/native O0/O2/Python/C++. The frozen external BST
adds one supported task; four other candidate storage probes do not solve their
full adapters and remain excluded from the corpus positives. Old task/oracle/golden
and baseline sources are unchanged.

[The full report](../../benchmarks/reports/2026-10-04/V5_COLLECTIONS.md) includes
per-task/per-language/per-tokenizer CSV, all three cuts plus isolated new tasks,
raw samples, quota boundaries and exact reproduction commands. Source density
worsens: external +21.9–31.3%, combined +15.6–23.7% versus Python. The old
185 verified language sources/counts match exactly; this is cohort movement.
Density does not establish model efficiency; historical generation remains 0/24
for NIL versus 11/24 Python. No inference, model training or Rust dependencies added.

Scalar-record build/update and retained-child update scale linearly; fresh/shared
nested builders and aliased builders are quadratic. Record-field append remains
superlinear. A same-IR, same-flags paired runtime A/B confirms **59% byte-append
regression**, attributed to out-of-line recursive root helpers; it is not hidden
as noise. Scan remains approximately unchanged. Bulk equality's prior plugin
regression remains outstanding and was not modified. All performance samples were
collected sequentially after functional campaigns, with 25 repeats and one warmup.

## Development findings and process

No runtime semantic divergence or memory-safety defect was found. Authored fixtures
initially used incorrect function labels/return types, noncanonical scalar arity and
unsupported &&/||; corrected fixtures passed without expanding grammar. Harness
integration tests caught stale task counts/upstream-fixture assumptions and missing
JSONL overlap metadata. Frozen contracts/goldens were never edited.

Earlier sanitizer/fuzz runs overlapped a manifest capability edit and old runners
reported E024; those version-mismatch runs were discarded. All final CI/sanitizer
and three-seed campaigns were repeated with stable code, including the strengthened
snapshot regression. An oversized shared-child timing control was stopped after
quadratic lower-size evidence; final measured sizes retain 25 samples each.

Conventional Commits are local only on feat/application-core and the benchmark
submodule branch. No push was performed. No further optimization or library
work automatically follows this delivery.
