# Round 25 — root ownership checkpoint

**R3 remains unimplemented. This round is partial.**

R3 was measured at its witness and refused rather than part-proved: the two
course-schedule copy sites are equal in size, only one is reachable without new
field-path aliasing analysis, and that one sits inside a lazy region. ADR 049's
dated R3 section records the measurement and the remaining work.

R1 and R2 are validated. The unchanged natural BST is 1,411.189 → 24.935 ms
(1.900× C++) and the unchanged natural balanced tree 1,481.530 → 27.151 ms
(2.020× C++). Natural qualification 19 → 21/150; tuned stays 23/150. Native
natural copied payload falls 400.057 → 240.057 GB (39.99%). The course-schedule
cliff is untouched at 200.886× C++ and needs R3.

[R1 report](../benchmarks/reports/2026-10-10/ROUND25_R1.md) and
[R2 report](../benchmarks/reports/2026-10-10/ROUND25_R2.md) include forecast
outcomes, negative-case matrices, timing/RSS, census deltas and all gates.
[R1 receipt](../benchmarks/reports/2026-10-10/ROUND25_R1.json) carries artifact
hashes and per-family counts.

R2 infers per-parameter ownership (`owns`) and return provenance (`origins`) over
a call-graph least fixed point, then surrenders a caller root at a call whose
argument has no remaining observation other than proved pre-call scalar
snapshots. Uniqueness is still decided at the mutation by the runtime root count,
so record child edges, second loop-state slots, independently retained returned
aliases and callee-side stores all still copy.

596 debug/release tests, 274 sanitizer tests, two full 88-chain-family seeds
(821 programs each) with zero divergences, zero non-resource divergences and zero
rule-3 failures; corpus 2,815 / original 610, natural/tuned 1,070 each, depth
audit 36/36. Nothing pushed.
