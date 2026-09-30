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
