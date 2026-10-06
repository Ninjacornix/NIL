# Round 18 — independent audit of ADR 038
## Verdict
**No unsound reconciliation case found.** This is an independent review and new seeded testing, not an exhaustive proof. Round 17's map/transform regressions and push call penalty are real outstanding performance findings; Parts A–C start only after this audit.
Reviewed main baseline `8647ba7`, benchmarks `0766a90`, on `feat/std-inline`. Runtime code was not changed during this audit.
## Soundness argument and restrictions
A frame's committed mirrors contribute exact physical root counts at observations. Stores update only intended slots. Enter synchronizes the caller; only the top frame can be dirty. Slow synchronization retains new roots before dropping old ones, with no allocation/collection during the transition. For acyclic record-child graphs, child edges count when a parent becomes live and drop when it becomes dead; the final rooted graph is the same as eager accounting. Unique graph updates synchronize before changing children and adjust child roots eagerly.
Leave drops committed roots and discards pending slot changes: no allocation follows inside that activation, and the returned value is adopted by the caller before another observation. Zero-root transfer gaps remain collection-free. Returning/register-held payloads are not prematurely freed. Growth relocation happens after synchronization and physical uniqueness, updates both the unique slot and its mirror, and keeps counts/capacity charges in step.
Emitter arrays reserve and initialize `2*count` pointers; runtime CLI frames reserve `2*slots+1`, including zero-slot inputs. Slot GEPs address the intended half only. `nil_roots_find` searches all frames only after synchronization; physical uniqueness prevents multiple active slot matches. Reusing a projected child with a live parent remains forbidden by child roots.
The global dirty flag is valid because callers cannot store while a callee runs and no native threads/embedded concurrent runtime exist. Plain globals are **not** a concurrency guarantee: an embedding or concurrency feature must redesign this state. Future runtime root/live readers must reconcile; this requirement remains load-bearing. Stack singleton virtual roots are a separate fixed scalar charge, never managed slot targets, and do not need reconciliation for their fixed root count.
## Independent commands and receipts
Logs and JSON under `/tmp/nil-round18/` stay outside git.
| Command | Result | Receipt |
|---|---|---|
| `./scripts/ci.sh` | 464 passed, zero warnings/failures | audit-ci-debug.log |
| `./scripts/ci.sh release` | 461 passed, zero warnings/failures | audit-ci-release.log |
| `./scripts/sanitize.sh` | 197 passed, zero warnings/failures | audit-sanitize.log |
| `cargo test -p nil-llvm --test roots --locked --offline` | 9 passed; native O0/O2 | audit-adversarial.log |
| `verify.py --output /tmp/nil-round18/audit-corpus.json` | 2,815 checks | audit-corpus.log/json |
| `verify.py --group original --output /tmp/nil-round18/audit-original.json` | exactly 610 | audit-original.log/json |

Full fuzz commands `./scripts/fuzz-v5.sh --seed SEED --out /tmp/nil-round18/audit-fuzz-SEED`:

| Seed | Programs | O0/O2 builds | Families | Divergences |
|---|---:|---:|---:|---:|
| 102061801 | 467 | 934 | 467 | 0 |
| 74018329 | 467 | 934 | 467 | 0 |
| 918273645 | 467 | 934 | 467 | 0 |

Per-code counts per seed: E009=3, E012=24, E013=33, E014=8, E015=8, E016=14, E017=10, E018=11, E019=3, E020=6, E021=5, E022=3, OK=339. Host stdout and files are compared, not only values.

Six new families: `reconcile_deep_pending_calls`, `reconcile_child_growth`, `reconcile_quota_success`, `reconcile_quota_failure`, `reconcile_cross_frame_relocation`, `reconcile_callee_collection`. Every family runs once per seed. Three native regression tests cover deep pending-store recursion, record-child growth, and the adjacent successful/failing copy-quota boundary. Existing alias, relocation, callee collection and virtual singleton tests remain. No failed seed or sanitizer finding was hidden.

Frozen corpus sources/tasks/oracles/baselines remain unchanged. No compiler change is accepted merely because aggregate tests pass; this audit established the observation-point invariant first.
