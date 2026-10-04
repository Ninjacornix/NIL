# Expr-v5 record validation

[ADR 028](../adr/028.md) was committed before implementation (`95db106`).
[Specification](../language/EXPR_V5.md#nominal-records-adr-028) defines layout,
immutable access/update, supported nesting and checked failures. Construction,
projection, update and typed record-map storage pass ADR 026's core criterion;
no algorithmic convenience was admitted. Intrinsic variants stay **32 → 32**.

## Reproducible gates

Run from the NIL root, with the benchmark submodule initialized:

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 240 --out /tmp/nil-records/fuzz-final-5130572
./scripts/fuzz-v5.sh --seed 1729 --cases 240 --out /tmp/nil-records/fuzz-final-1729
./scripts/fuzz-v5.sh --seed 8675309 --cases 240 --out /tmp/nil-records/fuzz-8675309
```

Debug/release each pass **311 tests**, no clippy/compiler warnings. Sanitizers pass
**105 tests** across application/keyed/numeric/records, clean ASan/UBSan and
float-cast-overflow checks; macOS LeakSanitizer remains disabled under the existing
policy. Fuzz compares **720 programs / 1,440 O0/O2 builds**, zero divergences.
Every one of 32 record families appears on each seed. Per seed:
`OK=171 E009=1 E012=14 E013=14 E014=5 E015=3 E016=10 E017=3 E018=4
E019=2 E020=5 E021=5 E022=3`. Static E023/malformed syntax has separate mutation
and validation tests. This is bounded seeded generation, not an exhaustive proof.

Named native tests verify constructor/projection/update, aliased and duplicate
dynamic leaves, nesting/calls/loops/recursion, lazy traps/effects and ordering,
scalar-record map special-value bits, missing/duplicate keys, sort-mode priority,
quota retention/reclamation and entry-wrapper requirements. IR assertions verify
inline LLVM product operations rather than a new opaque runtime call. Earlier
profile parser fixtures and excessive-nesting regressions remain passing.

The full frozen corpus passes **2,625 checks**, and the original **610** are
separately reconfirmed. There are **61 verified NIL references, 14 unsupported**;
Go counting is the only new verified unblock. Existing sources, tasks, oracles,
goldens and baselines stay unchanged. All commands, exact output receipts,
source hashes, per-code/call-family counts and remaining blockers are in the
[benchmark report](../../benchmarks/reports/2026-10-04/V5_RECORDS.md) and
[receipts](../../benchmarks/reports/2026-10-04/V5_RECORD_VALIDATION.txt).

## Limits and honest findings

No fixed-task token saving was forecast or observed. The new Go reference worsens
external source density to **12.9–23.3% more than Python**, combined **8.0–16.8%**.
Source density does not overturn NIL 0/24 versus Python 11/24 generation results.
Sequence-bearing record map values, record buffers, recursive records, native
record entry marshalling and shortest float formatting remain deferred.

Twenty-five-repeat performance controls: scan16MiB **7.573 ms**, append1MiB
**24.626 ms**, transform16MiB **34.991 ms**. The report includes baseline times,
raw distributions and comparisons; no clear regression established. Scalar-record
map insertion remains non-quadratic, with Pair(i,i)'s tested quota boundary
**262,144 succeeds / 262,145 E013**. Whole-record liveness keeps child aliases
conservative: record-field append is still superlinear (**3.653 / 16.955 / 396.285 ms**
at 10k / 40k / 160k). No existing mutation proof was weakened to hide that cost.

A parser helper expansion overflowed the debug stack in the existing nesting
regression; splitting helpers restored the unchanged depth limit. Review also
caught incorrect sort-mode error priority, fixed with a regression. Initial Go
adapter mistakes were fixed before admission. Final sanitizer/fuzz campaigns found
no further runtime defect. Commits are local only; neither repository was pushed.
