# Architecture decision records

“Accepted” records a settled constraint; “proposed” is a documented working choice,
not an implemented feature or final ABI. Update or supersede a record when evidence
changes. Original research remains under `docs/about/misc/`.

- [ADR-001: Rust implementation](001.md) — Accepted.
- [ADR-002: Initial execution backend](002.md) — Accepted for M1; production backend unresolved.
- [ADR-003: Semantic HIR boundary](003.md) — Accepted boundary; proposed M1 shape.
- [ADR-004: MIR and SSA](004.md) — Proposed; implementation deferred to M4.
- [ADR-005: Type system and integer semantics](005.md) — Proposed M1 policy.
- [ADR-006: Initial parser and surface profile](006.md) — Proposed experiment; no syntax winner selected.
- [ADR-007: Plugin semantic boundary](007.md) — Accepted fixed-grammar constraint; ABI proposed.
- [ADR-008: Canonicalization](008.md) — Accepted principle; algorithm/format deferred.
- [ADR-009: Diagnostic contract](009.md) — Proposed M1 contract.
- [ADR-010: Measurement and scope gates](010.md) — Accepted methodology; experiment thresholds open.

- [ADR-011: Structured control-flow regions](011.md) — Accepted for M2; surface spelling experimental.

- [ADR-012: LLVM native default](012.md) — Accepted after explicit user selection; portable MIR/production hardening remain open.

- [ADR-013: expr-v3 machine integers](013.md) — Accepted opt-in experiment; wrapping arithmetic and optional native accounting.

- [ADR-014: expr-v4 typed values and fixed arrays](014.md) — Accepted implementation experiment; model/TCR and systems-memory decisions remain open.

- [ADR-015: private sparse loop storage](015.md) — Accepted implementation experiment; preserves immutable arrays and guarded update semantics.

- [016: external benchmark suite and submodule](016.md)

- [ADR 017: application sequences and effects](017.md) — Accepted experiment; runtime ownership and host boundaries.

- [ADR 018: checked application runtime performance](018.md) — Accepted v5 implementation; LTO, bulk reads and conservative root retention.

- [ADR 019: rootless scalar lazy regions](019.md) — Accepted v5 proof; retains laziness and conservative unproved paths.

- [ADR 022: corpus-driven sequence operations](022.md) — Accepted experimental equality, search and bulk canonical parsing.

- [ADR 023: ordered maps before records](023.md) — Accepted v5 implementation; corpus measurements recorded.

- [ADR 024: retain syntax after token attribution](024.md) — Accepted analysis outcome; no surface change, larger algorithm-abstraction decision remains open.

- [ADR 025: deterministic sorting and snapshot iteration](025.md) — Preregistered experiment with corpus-derived forecasts and immutable semantics.

- [026 — Core/extension boundary](026.md)
- [027 — Numeric semantics](027.md)

- [028 — Immutable records and binding core boundary](028.md)
- [029 — Validated semantic plugin boundary](029.md)
- [030 — Acyclic dynamic record buffers and index links](030.md)
