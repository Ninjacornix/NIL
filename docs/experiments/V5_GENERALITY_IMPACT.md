# Pre-implementation v5 generality ranking

Frozen before implementation. Compiler base `5391b3a`; original verified density corpus and oracles from benchmark `9be44d6`. No task, oracle, golden or baseline is changed. Candidate sources below are **unverified estimates**, never adaptation positives. Counted with the same pinned vocabulary-only Gemma/Qwen and tiktoken cl100k/o200k backends, without inference.

## Ranking and decision

1. **`parsebuf(s,s)->v`**: parse canonical signed i64 fields separated by any byte in an explicit separator set. Targets six numeric tasks; removes delimiter scans, pre-counting, repeated parse/slice operations and manual buffer construction. Estimated savings 864 Gemma / 650 Qwen / 623 cl100k / 633 o200k tokens. This is a generic typed bulk conversion, not a named task or fused min/max/dot operation.
2. **`find(sequence,i64,i64)->i64`**: find an element at/after start; return length when absent. Enables explicit delimiter iteration without nested sequence types, views or new grammar. Four candidate tasks save 140/98/97/101 tokens. It does not eliminate all slice copying or repeated search; do not rewrite other tasks merely to claim coverage.
3. **`equal(sequence,sequence)->bool`**: compare matching dynamic sequence types. Adjacent-line dedupe saves 48/36/36/35 tokens; low implementation/semantic cost and allocation-free.

Implement these three existing-intrinsic-form operations first. They require no new types, records, multi-result syntax or workspace dependencies. Existing arithmetic, loops and concat remain explicit. Estimates sum to 1052/784/756/769 fewer tokens, projecting paired NIL totals 2144/1634/1582/1571 versus Python 2227/1728/1705/1712. This predicts approximately 3.7–8.2% fewer tokens than Python, **conditional on correct candidate programs**. Report actual deviations; reject unverified predictions as findings.

## Deferred alternatives

- Dynamic push/builder: numeric corpus construction is removed more directly by parsebuf; text concat is already amortized. A push-only numeric parser still needs scans/slices/canonical conversion. Defer a new ownership/reuse contract until a measured remaining incremental numeric task warrants it.
- Multi-result bindings/records: would reduce repeated min/max parsing, but parsebuf removes that problem across six tasks with one typed operation. New HIR values, signatures, syntax and lifetime invariants cost much more. No current task requires records to produce its correct output.
- Split into lines: requires nested sequence storage and additional quota/alias rules. Find provides the iteration boundary using existing types; no list-of-bytes type yet.
- Environment access: resolves one unsupported task but has no paired density saving to estimate and requires a Host/environment portability contract. Keep the unsupported task and document the capability boundary.
- Format/join/Unicode operations: possible later candidates, not justified by this ranking. No spelling or identifier compression.

## Proposed contracts and safety boundaries

`equal`: matching v/v or s/s, no allocation/effects. `find`: v or s plus element and start, no allocation/effects; start outside [0,length] E012, then out-of-range byte needle E014. `parsebuf`: s text and s separator bytes, newly allocated exact-capacity v; empty text gives an empty buffer, one final separator is allowed, interior/leading empty fields are E016. Reserve output capacity after counting fields and before parsing, so E013 precedes E016 when both apply. Canonical conversion is exactly parse, including i64 extrema; no whitespace/plus/leading zeroes. Arguments evaluate left-to-right.

Equality and find extend the allocation-free borrowing summary. Parsebuf allocates a different element-width result and cannot reuse its byte input; keep existing roots and quota checks. Subsequent buffer concatenation/replacement must retain their existing uniqueness proof and immutable aliases. Add composition/alias/lazy failure tests instead of claiming allocation-free conversion.

## Per-task estimates

| Candidate | Task | G before/after | Q before/after | C before/after | O before/after |
|---|---|---:|---:|---:|---:|
| parsebuf | buffer_sum | 231/61 | 177/47 | 172/47 | 171/46 |
| parsebuf | sum_integers | 109/61 | 82/47 | 80/47 | 81/46 |
| parsebuf | prefix_sums | 245/74 | 188/57 | 183/57 | 182/56 |
| parsebuf | min_max | 282/98 | 209/75 | 205/75 | 207/76 |
| parsebuf | sum_pairs | 163/70 | 122/56 | 119/56 | 120/55 |
| parsebuf | buffer_dot | 338/140 | 258/104 | 250/104 | 254/103 |
| equal | dedupe_adjacent_lines | 272/224 | 203/167 | 199/163 | 201/166 |
| find | first_fields_csv | 149/98 | 111/77 | 108/74 | 108/73 |
| find | number_lines | 147/93 | 114/74 | 111/72 | 111/71 |
| find | nonempty_lines | 114/90 | 87/70 | 85/68 | 86/67 |
| find | slice_file | 114/103 | 81/74 | 79/72 | 80/73 |

## Estimated sources (not verified)

### parsebuf: buffer_sum

```text
(s,s)=!write(b,!concat(!format(b(!parsebuf(!read(a),"\n"))),"\n"))
(v)=@(a,0,0;b<#a;a,b+1,c+a[b];c)
```

### parsebuf: sum_integers

```text
(s,s)=!write(b,!concat(!format(b(!parsebuf(!read(a),"\n"))),"\n"))
(v)=@(a,0,0;b<#a;a,b+1,c+a[b];c)
```

### parsebuf: prefix_sums

```text
(s,s)=!write(b,b(!parsebuf(!read(a),"\n")))
(v):s=@(a,0,0,"";b<#a;a,b+1,c+a[b],!concat(d,!concat(!format(c+a[b]),"\n"));d)
```

### parsebuf: min_max

```text
(s,s)=!write(b,b(!parsebuf(!read(a),"\n")))
(v):s=@(a,0,a[0],a[0];b<#a;a,b+1,a[b]<c?a[b]:c,a[b]>d?a[b]:d;!concat(!format(c),!concat(",",!concat(!format(d),"\n"))))
```

### parsebuf: sum_pairs

```text
(s,s)=!write(b,b(!parsebuf(!read(a),",\n")))
(v):s=@(a,0,"";b<#a;a,b+2,!concat(c,!concat(!format(a[b]+a[b+1]),"\n"));c)
```

### parsebuf: buffer_dot

```text
(s,s)=!write(b,!concat(!format(b(!parsebuf(!read(a),",\n"))),"\n"))
(v)=d(c(a,0),c(a,1))
(v,i):v=@(a,b,!buffer(#a/2,0),0;d<#c;a,b,c[d:a[d*2+b]],d+1;c)
(v,v)=@(a,b,0,0;c<#a;a,b,c+1,d+a[c]*b[c];d)
```

### equal: dedupe_adjacent_lines

```text
(s,s)=!write(b,b(!read(a)))
(s):s=@(a,0,0,"","",false;b<#a;a,b+1,a[b]==10?b+1:c,a[b]==10?(f?(!equal(!slice(a,c,b-c),e)?d:!concat(d,!concat(!slice(a,c,b-c),"\n"))):!concat(d,!concat(!slice(a,c,b-c),"\n"))):d,a[b]==10?!slice(a,c,b-c):e,a[b]==10?true:f;b>c?(f?(!equal(!slice(a,c,b-c),e)?d:!concat(d,!concat(!slice(a,c,b-c),"\n"))):!concat(d,!concat(!slice(a,c,b-c),"\n"))):d)
```

### find: first_fields_csv

```text
(s,s)=!write(b,b(!read(a)))
(s):s=@(a,0,"";b<#a;a,!find(a,10,b)+1,!concat(c,c(!slice(a,b,!find(a,10,b)-b)));c)
(s):s=!concat(!slice(a,0,!find(a,44,0)),"\n")
```

### find: number_lines

```text
(s,s)=!write(b,b(!read(a)))
(s):s=@(a,0,"",1;b<#a;a,!find(a,10,b)+1,!concat(c,!concat(!format(d),!concat(":",!concat(!slice(a,b,!find(a,10,b)-b),"\n")))),d+1;c)
```

### find: nonempty_lines

```text
(s,s)=!write(b,b(!read(a)))
(s):s=@(a,0,"";b<#a;a,!find(a,10,b)+1,!concat(c,c(!slice(a,b,!find(a,10,b)-b)));c)
(s):s=#a==0?"":!concat(a,"\n")
```

### find: slice_file

```text
(s,s)=!write(b,b(!read(a)))
(s):s=c(a,!find(a,10,0))
(s,i):s=@(a,b,0;a[c]!=44;a,b,c+1;!slice(a,b+1+!parse(!slice(a,0,c)),!parse(!slice(a,c+1,b-c-1))))
```

## Validation plan

Add HIR signature rejection, reference/native O0/O2 successes and failures, quota priority, call/loop aliases, lazy traps/effects, immutable concat composition and rootless proof tests. Extend fuzz families for every operation, canonical boundaries and E012/E013/E014/E016 ordering. Run debug/release CI, ASan/UBSan, three seeded differential campaigns, unchanged 610-check corpus and all four tokenizer counts. Re-run the existing 16 MiB scan/transform and 1 MiB append measurements with the established benchmark harness. Commit locally only.
