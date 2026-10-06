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
| `./scripts/ci.sh` | 358 passed, zero warnings |
| `./scripts/ci.sh release` | 358 passed, zero warnings |
| `./scripts/sanitize.sh` | 135 passed, all six suites; ASan/UBSan/float-cast checks clean |
| `./scripts/fuzz-v5.sh --seed 1729 --cases 308 ...` | 308 programs, 616 O0/O2 builds, zero divergences |
| Same with 8675309 and 5130572 | 308/616/zero each |
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

Per fuzz seed: **OK222, E0092, E01218, E01322, E0145, E0154, E01610, E0173,
E0185, E0193, E0206, E0215, E0223**. Aggregate is triple. All 44 collection
families execute once per seed; JSON includes every call-family and mutation count.
This is bounded template coverage, not exhaustive arbitrary recursive/nested types.
Some source substitutions do not apply to a template; mutation counts do not imply
that every mutation was rejected. No final diagnostic-code/output divergence occurred.

## Load-bearing proof boundaries

Validated earlier-type IDs and depth ≤32 make the dynamic ownership graph acyclic.
Word-slot offsets are checked against nominal definitions (≤4096 logical slots).
Physical row width is 8*max(1, logical slots), preserving empty logical fields.
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
nested builders and aliased builders remain quadratic. Record-field append remains
superlinear (160k: 409.22 ms, earlier 322.69 ms); its revision-dependent A/B loss
is disclosed and not isolated further. Bulk equality's ~16.96 ms prior plugin
regression is outstanding and unchanged.

**Ordinary byte append is restored**: paired pre-fix/current 39.158→24.540 ms;
historical/current 24.463→24.545 ms. Final standard controls are 24.764 ms append,
7.836 ms scan and 37.922 ms transform. Transform's runtime A/B is 40.289→39.878 ms,
not evidence of an unavoidable child-root elevation. All samples use 25 repeats
and one warmup, collected sequentially after functional campaigns, without CPU
isolation. The report preserves every loss, full curves and raw samples.

Only a validated whole-program scan proving absence of record buffers selects
the ordinary root/quota path. It visits signatures, nested lazy/loop instructions,
reachable record fields, every callee and plugin providers. Typed record-buffer
and mixed programs keep transitive child tracking; no proof/check was weakened.
Zero-slot rows use an initialized physical word while fields stay logically empty.
The exact `record Z(x:0)` / `=#!buffer[Z](2,Z([]))` program returns 2 in reference,
O0/O2 and sanitized O0/O2; [failure and fix evidence](../../benchmarks/reports/2026-10-04/V5_COLLECTION_ZERO_LAYOUT.json)
includes the retained pre-fix UBSan abort and full reproduction commands.
Twenty-one documented commands pass, with add/weighted/reverse sources unchanged.

## Development findings and process

The final audit found a real native memory-safety defect: zero-slot record buffers
divided by zero, including an O0/O2 process divergence. The overseer reproduced
exit 133 at O2; the local UBSan reproduction reported division by zero. The earlier
clean campaigns omitted this whole layout class. The continuation pads physical
rows to one initialized word, preserves zero logical fields, and adds 20
degenerate-layout families across buffers, maps, aliases and loop state. Authored fixtures
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
