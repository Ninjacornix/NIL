# Round 29 — finishing keyed collections

## Unfavourable first

**The two cohort rewrites did not happen, and the reason matters more than the
omission: the sampled natural cohort has no problem that wants a hash map.**
The two best candidates both already use dense direct-address buffers, which
are strictly better than hashing for their key ranges —
`longest-substring-without-repeating-characters` indexes a 256-entry buffer by
byte value, and `target-sum` indexes a sum-offset buffer. Rewriting either with
`!imap` would cost tokens *and* speed. `contains-duplicate`, rewritten in Round
28, was the only hash user in the cohort, and it wanted a **set**, not a map.
So the integer-keyed map ships with a control measurement and no cohort
evidence that it earns its place.

**No problem newly qualifies.** Natural stays **21/150** and tuned **23/150**,
unchanged for a fifth round. Keyed collections have now consumed two rounds
without moving the number they were meant to move.

**The set shipped in Round 28 with no differential coverage at all.** The audit
caught it: the campaign sat at 843 programs while the set had only unit tests,
so two rounds of "zero non-resource divergences" said nothing about it. Fourteen
families later it is 857, and nothing was found — but the gap was real and mine.

**A pointer-type defect shipped too.** `nil_copy_hint("map_insert_key", …, 2,
key)` passed raw key bytes through a parameter declared `const NilSequence *`
after the raw-key refactor. It was harmless only because role 2 never
dereferences the source, and C short-circuits `role==1 && …`. Harmless by
accident is not harmless by design.

**`!set(seq)` was built before it was specified.** ADR 051 addendum B records
its semantics after the fact and says so.

## Forecast versus actual

| Measure | Forecast | Actual |
|---|---|---|
| map control vs `unordered_map` | ≈1.4× (set was 1.383×) | **1.441×** |
| spelling candidates measured | ≥3 | **5**; four tied at 48 cl100k |
| map operations | get/put/has/remove/size/iterate | **all six**, insertion order |
| set fuzz families | cover the audit gap | **14**, campaign 843 → 857 |
| map fuzz families | as for the set | **8**, campaign → **865** |
| cohort rewrites | 2 | **0** — no cohort problem wants a map |
| newly qualifying problems | 0–2 | **0** |

## natural-v2

Unchanged from Round 28; no program was added this round.

| Program | v1 | v2 loop | v3 bulk | Python | v3 vs v1 | v3 vs Python | v3 / C++ |
|---|---:|---:|---:|---:|---:|---:|---:|
| contains-duplicate | 51 | 45 | **24** | 14 | **−52.9%** | **+71.4%** | **1.829×** |

## What shipped

**`!imap()`**, an integer-keyed, integer-valued map on the raw-key path, so the
emitter passes a plain i64 and nothing is allocated per call. **1.441×
`std::unordered_map`** inserting 200,000 keys over 15 interleaved pairs.

Spelling came from five measured candidates on a two-sum style kernel: `!imap`,
`!table`, `!dict` and a fresh-verb set tied at **48** cl100k, `!map2` cost 49,
o200k agreeing throughout. The tie broke on grammar, as in Round 26: `!imap`
reuses `put`/`get`/`has`/`remove`/`size`/`key`, so **no new verb enters the
language**, while the fresh-verb candidate would have spelled insertion `!set`
and collided with the set constructor.

The change that made this cheap is that validation's shared keyed-operation arm
now derives the key type from the collection instead of hardcoding `Bytes`, so
these verbs **generalize** rather than acquiring a special case. `!key` returns
the key in its declared type — an i64 here, eight bytes for the byte maps.
Iteration is insertion order, removal preserves it, and a missing key faults
**E019** rather than defaulting, at both optimization levels.

**Twenty-two new differential families**, taking the campaign from 843 to
**865** programs, byte-identical at `--jobs 1` and `--jobs 5`.

## Gates

Fast gates between steps; the closing full gate is the overseer's. Last fast
receipt `/private/tmp/r29-f2/receipt.json`, every gate exit 0: ci **653**,
sanitizer **313**, fuzz seed 61001 **865 programs** with 0 non-resource
divergences and 0 rule-3 failures, corpus **2,815**, both cohorts
**1,070/1,070**, depth **36/36**. No count decreased.

## Commits

```
fdb73d0 docs(collections): record !set(seq) semantics as an after-the-fact addendum
d46671d test(fuzz): add fourteen differential families for integer-keyed sets
0c18ac9 feat(collections): add an integer-keyed map on the raw-key path
```

## What the next round should weigh

Keyed collections are finished and have moved qualification by zero. The
discovery batch of held-out problems is the right next step, and it should be
read as a test of whether the nested-child-edge work is worth doing at all:

* **If the batch is full of hash-keyed problems**, this round's map earns its
  place retroactively and Part 1 stays deferred.
* **If the batch is dominated by nested containers**, Part 1 — interprocedural
  child-edge transfer, and the `--explain-copies` blind spot at `roots == 2`
  that makes every earlier copy figure unreliable — becomes the priority, with
  `course-schedule` v2 the known target at an honest ceiling near 4× C++.
* **If neither**, the 3× bar itself is the thing to re-examine, because five
  rounds of compiler work have not moved 21/150.
