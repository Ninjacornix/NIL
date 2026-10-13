# Round 28 — nested-data ownership, integer-keyed maps and sets

## Unfavourable first

**Part 2 is a third shipped, not shipped.** The integer-keyed **set** has its
frontend, typing and reference semantics, and is refused natively with E009 on
every path. The **integer-keyed map** is not started. So there are **no cohort
rewrites, no token-versus-Python table and no speed numbers**: all of that needs
native execution, which needs an integer-hash path in the runtime table. The
measured spelling win of −11.8% is a source measurement, not a collected one.

**Part 1 is not started.** ADR 051 and its addendum carry the diagnosis only.

**Two defects in my own Round 27 scheduler, found by running the full gate
after adding one test target.** Permits were acquired one at a time, so two
wide gates could each hold a partial allocation and wait on the other: a full
run measured **10,481.7 s wall against 2,622.2 s of gate time**, almost eight
thousand seconds of starvation. And the two cargo gates write `target/`, so the
depth audit **failed in 0.2 s** because `rustc` linked `target/release/*.rlib`
while cargo was rewriting it — the same class of race as the fuzz binary being
replaced mid-exec, which the audit had just caught. Both are fixed; a full run
is green again at **1,400.9 s**.

That is the third target-directory race in two rounds. The pattern is worth
naming: any gate that reads a build artifact races every gate that runs cargo.

## What landed

### Part 2, the integer-keyed set

The spelling is the one measurement chose: `!set()` with `!add`, which cost
**45 cl100k tokens against 51** for byte keys via `!format` on the
`contains-duplicate` kernel, with cl100k and o200k agreeing on every candidate.
Signature letter `k`, matching `m`, `t`, `v`, `s`. Intrinsics rise **35 → 38**
with `Set`, `Add` and `Remove`; `Has` and `Size` now accept any keyed
collection through a new `Type::key_type`, so a set keys on an i64 and a map
still keys on bytes.

Keys are the eight big-endian bytes of the i64 internally, which is injective,
so the whole range works: `i64min`, `i64max` and negatives are distinct and
none aliases a positive. Insertion order stays the contract and removal
preserves it; the encoding is not an ordering claim.

Covered by tests: membership and size with no formatting, idempotent re-add,
absent keys false rather than trapping, the whole i64 range, removal of
present, absent and last keys, aliases keeping their contents through both add
and remove, `contains-duplicate` including negatives and `i64min`, typing that
rejects a byte key on a set and an integer key on a map, and the native refusal
on both the `entry_runtime` and `build_internal` paths.

### Forecast versus actual, so far

| ADR 051 forecast | Actual |
|---|---|
| `contains-duplicate` kernel 51 → 45, −11.8% | spelling confirmed at 45; **not yet collected**, since the program cannot run natively |
| two further problems 5–15% fewer tokens | not attempted |
| map/set controls 1.5–4× C++ | not measurable yet |
| newly qualifying problems 0–3 | **0** |

## Gates

`scripts/gates.sh full --out /tmp/r28-p2b`, receipt
`/private/tmp/r28-p2b/receipt.json`, every gate exit 0, **1,400.9 s**:

| Gate | Result |
|---|---|
| ci / ci release | **646** tests each, 0 warnings |
| sanitize | **306** tests, 0 ASan/UBSan findings |
| fuzz 61001 / 83003 | **843** programs each, 0 non-resource divergences, 0 rule-3, 0 retries |
| corpus | **2,815**, 0 inconclusive; original **exactly 610** |
| cohorts | tuned **1,070/1,070**, natural **1,070/1,070** |
| depth | **36/36** |

Tests rose 639 → 646 and sanitizer 305 → 306; nothing decreased.

## Commits

```
c381cd1 docs(collections): preregister ADR 051 for nested ownership and integer keys
adfd0d0 docs(collections): quantify the nested-ownership copy and show it is interprocedural
409aa4e feat(collections): add an integer-keyed set, frontend and reference first
ace00e8 fix(gates): serialize admission and keep cargo gates off shared target
```

Branches `feat/core-collections` and submodule `exp/core-collections`. Nothing
pushed.

## Next, in order

1. Native lowering for the set: an integer-hash path in the runtime table, then
   the cohort rewrites and the measurements Part 2 promised.
2. The integer-keyed map, which shares that path.
3. Part 1a, fixing `--explain-copies` so it reports the 32,584 copies it
   currently misses, before Part 1b uses it.
4. Part 1b, interprocedural child-edge transfer.
