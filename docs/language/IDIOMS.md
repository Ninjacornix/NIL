# NIL idioms (expr-v5), drafted from the Round 30 discovery batch

Every idiom below appeared in a solution that passes the frozen fixtures, and
every trap below cost a real attempt. Token costs are cl100k on the kernel cut;
o200k agrees within a token or two unless noted.

## The scaffold

Two lines of file adapter, unchanged across problems:

```text
(s,s)=!write(b,b(!read(a)))
(s):s=!concat(!format(c(!parsebuf(a," \n"))),"\n")      scalar output, 22
(s):s=c(!parsebuf(a," \n"))                             vector output, 12
```

Functions are referenced by declaration order — `a` is function 0, `b` is
function 1, and so on — and a name followed by `(` is a **call** while a bare
name is a parameter or loop slot. Both readings coexist safely: a four-slot
loop can still call function 1 as `b(x)`.

## Idioms that worked

```text
!slice(a,1,a[0])                     strip the leading count, 11
@(a,1,1,0;d<a[0];a,c,b+c,d+1;b)      scalar fold with carried input, 22
#@(…;b)                              length of a loop-built buffer, 3
b[c:x]                               functional replace at an index, 6
!concat(b,!buffer(1,x))              append one value, 11
!sort(buf,0)                         ascending sort, 7
!put(m,k,!has(m,k)?!get(m,k)+1:1)    frequency count on !imap, 24
@(m,0,0;c<!size(a);a,…!key(a,c)…)    iterate a keyed collection in order, 16
```

Sorting by two keys has no direct spelling, so pack them into one i64 and sort
that: `(1000000-freq)*4000001+value+2000000` sorts by descending frequency then
ascending value, and the value is recovered with
`x-x/4000001*4000001-2000000`. This is the whole reason
`top-k-frequent-elements` fits in 206 tokens rather than needing a comparator.

## Adapters NIL makes you write

These are the token hot spots. Both are fixed costs that Python gets from its
standard library, and together they explain most of the gap on vector-output
problems.

| Adapter | NIL | Python | Ratio |
|---|---:|---|---:|
| Row join | **40** | `' '.join(map(str,x))` — 7 | **5.7×** |
| Tolerant integer scan | **169** | `data.split()` — 3 | **56×** |

```text
(v):s=@(a,"",0;c<#a;a,!concat(b,!concat(c==0?"":" ",!format(a[c]))),c+1;!concat(b,"\n"))
```

## Traps

**`!parsebuf` cannot parse an empty vector.** The corpus encodes an empty
vector as `"0 \n"`, and every separator choice leaves an empty field, which
fails **E016 invalid decimal i64**. No admitted problem had ever hit this, so
the gap was invisible for thirty rounds. The only workaround is the 169-token
scanner above. This blocks four numeric problems in the batch outright.

**Loop state slots are renamed positionally.** A loop's state is always `a`,
`b`, `c`, … in order, so a function parameter you need inside the loop is
shadowed unless you thread it through the state. In `top-k-frequent-elements`
the parameter `b` was `k` outside the loop and the output buffer inside it, and
the mistake surfaced only as `E007 expected:I64 got:Buffer`. Thread it:
`@(a,b,!buffer(0,0),0;d<b;a,b,…,d+1;c)`.

**Loop state is parallel, not sequential.** Every step expression sees the old
state, so a value computed in one slot cannot be read by another in the same
iteration. Anything needed twice must be written twice. This is what makes a
monotonic deque impractical to write naturally — the new tail index is needed
by three other slots — and it is why `sliding-window-maximum` stayed at the
quadratic formulation.

**The default result type must be omitted.** Write `(v,i)=`, never `(v,i):i=`,
which is **E001 omit the default i64 result type**.

**All-i64 parameter lists use numeric arity.** Write `1=a*2`, not `(i)=a*2`,
which is **E001 use numeric arity for all-i64 parameters**.

**Functional replace on a live buffer copies it.** `a[i:x]` inside a loop whose
buffer is also read elsewhere copies the whole buffer per iteration. In
`redundant-connection` this made union-find **137.5× C++** with **98.8% of
samples in `_platform_memmove`** — and `--explain-copies` reported nothing,
because of the `roots == 2` blind spot recorded in ADR 051 addendum A.
