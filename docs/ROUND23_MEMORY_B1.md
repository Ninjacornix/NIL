# Round 23: ordinary-memory removal checkpoint (B1)

B2 ownership admission, std swap/pop and configurable depth/stack safety are not
implemented at this checkpoint. No natural source was tuned. B1 alone does not
remove the natural ownership cliffs. The removal forecast also missed: most
accounting serves garbage collection and immutable aliases, not just E013 parity.

## Decision and implementation

ADR 046 was committed before code (`f362c22`, inventory addendum `e5b0121`).
It supersedes the logical ledger and deterministic quota. Superseded uncommitted
ledger work is retained in `stash@{0}` (`superseded by ADR 046`), not merged.
The committed fail-fast acquisition change `b23d0e9` remains.

The v5 managed-storage guard defaults to 256 MiB, not a semantic quota or RSS
limit. Rust `Limits.memory_bytes` / LLVM `Options.memory_bytes` and CLI
`--memory-limit BYTES|NMiB` configure it; zero disables it. Invalid allocation
size, guard exhaustion or a detected native allocation failure reports E013.
Actual OS OOM/Rust allocator aborts cannot universally become clean diagnostics.
All other defined results, effects and diagnostics retain their meaning.

Removed hypothetical copy admission from unique replacement, capture/replay,
concat, sort, map and record updates. Removed activation-local singleton charges
and their release call. Capacity growth guards additional actual storage; copying
fallbacks guard their allocations. Geometric growth no longer budgets an imaginary
future old/right/result copy. Reference Arc uniqueness protects external Rust
aliases; native root uniqueness protects caller/parent aliases.

Root slots/mirrors, deferred reconciliation, recycled slots, child edges, SSA
pruning, reachable-storage estimates, collection heuristics and malloc/size checks
remain for physical safety and immutable values. No ledger or descriptors added.
The HIR byte-literal 64 MiB admission ceiling is gone; representability and compiler
source-size budgets remain. Direct FileHost reads use the new default rather than
an inherited 64 MiB convenience cap (new sparse-file regression).

## Testing policy

Every clean E013 comparison is **inconclusive**, even if both sides fail at the
same span. The fuzzer retains resource programs and O0/O2 observations outside git,
counts them per seed, and compares stdout/files/fault receipts exactly otherwise.
Signals, malformed/reversed/noisy diagnostics and sanitizer findings fail closed.
Native entry-admission E013 without a span is valid; it is not a fabricated span.
Corpus runners name resource checks and never count them as verified solutions.

The permanent 14-family fault audit still requires `--require-parity`; historical
resource predictions are not mandatory outcomes. Non-resource codes/spans and
prefix/EIO receipts remain checked. Its resource cases are listed separately.

## Inventory and validation receipts

The committed inventory is [ADR 046](adr/046.md). The measured inventory, per-file
net lines, renamed/converted tests, final gate outputs, census and timings are
reported in `benchmarks/reports/2026-10-08/ROUND23_B1.md` and its JSON companion.
Raw commands, logs, resource witnesses and generated binaries stay outside git at
`~/.local/share/nil/round23-memory/ordinary-b1/`. Reports record their hashes.

Final gates: **543** debug/release tests, **244** sanitizer tests, two full
**687**-family seeds with zero non-E013 divergences / **88** inconclusive
comparisons each, corpus **2,815**, original **610**, natural/tuned **1,070** each.
Natural qualification is unchanged at **19/150**; tuned **22/150**. The symmetric
concat RHS-root assertion passed the final O0/O2, sanitizer and fuzz gates.
ADR 043’s call-depth typo is corrected to E008; old intrinsic counts are historical.

One overloaded repeated correctness run timed out on the largest Last Stone
Weight reference case (120 seconds). It remains archived as a **failed** check,
never reclassified as resource-inconclusive. Final verification and timings run
without competing CI, sanitizer, fuzz or census jobs. No sanitizer finding or
non-resource semantic divergence has been observed.

## Remaining steps

B2 must use a general SSA/dependency proof to lift quota-only barriers; alias,
trap, effect and bounded-tick protections remain. G1 still needs returned/child
provenance and is not automatically proved by deleting quota checks.
B3 forecasts remain `!swap(a,b,c)` = 6 tokens and `!pop(a)` = 4 in both tokenizers,
saving 3/6 against the functional forms; neither is shipped here. Current call
depth remains 256; B4's proposed 1024 default/private-stack protection is not
implemented. Unbounded native stack exhaustion still lacks the promised E008
safety guarantee. Held-out problems remain held out. Nothing was pushed.
