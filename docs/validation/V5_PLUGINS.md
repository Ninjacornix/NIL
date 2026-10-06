# Expr-v5 plugin validation

[ADR 029](../adr/029.md) was committed first (`eea2e10`). The implemented boundary
links trusted local validated NIL bodies into HIR; it does not load arbitrary native
code or promise a sandbox. One equality implementation moved to a shipped provider;
`!equal` and the legacy HIR input remain compatible. One plugin-only `Packet` update
works through the documented manifest, evaluator and native build (output `42`).
Intrinsic variants stay 32; 31 executable core implementations plus compatibility alias.

## Gates and receipts

```sh
./scripts/ci.sh
./scripts/ci.sh release
./scripts/sanitize.sh
./scripts/fuzz-v5.sh --seed 5130572 --cases 264 --out /tmp/nil-plugins/fuzz-final-5130572
./scripts/fuzz-v5.sh --seed 1729 --cases 264 --out /tmp/nil-plugins/fuzz-final-1729
./scripts/fuzz-v5.sh --seed 8675309 --cases 264 --out /tmp/nil-plugins/fuzz-final-8675309
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group all --output /tmp/nil-plugins/verification.json
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-plugins/original-verification.json
```

[Actual command receipts](../../benchmarks/reports/2026-10-04/V5_PLUGIN_VALIDATION.txt):
**331 tests** each debug/release, zero warnings; **116** clean ASan/UBSan tests across
application/keyed/numeric/plugins/records (existing macOS LeakSanitizer exclusion).
Three seeds compare **792 generated programs / 1,584 O0/O2 builds**, zero divergences.
Each seed hits **OK190 E0092 E01214 E01317 E0145 E0154 E01610 E0173 E0184 E0192
E0205 E0215 E0223**. All 24 new plugin/equality families appear per seed, with values
crossing calls/loop state, old aliases, quota retention/reclamation, lazy traps/effects
and ordering. E008 bounded caller accounting and E024 contract/source failures have
separate unit/mutation coverage. This is bounded randomized generation, not exhaustive
verification of arbitrary providers. [Full counts](../../benchmarks/reports/2026-10-04/V5_PLUGIN_FUZZ.json).

Frozen corpus: **2,625 checks**, including the separately reconfirmed original **610**;
61 supported references/14 unsupported unchanged. No tasks/oracles/goldens/baselines or
earlier profile meanings changed. Corpus ran before the final generic std-alias test
addition; compatibility behavior used by all corpus sources stayed unchanged.

Native parity tests verify every type crossing the record boundary, shared dynamic
leaves, held aliases and quota, provider-internal checked traps, argument trap priority,
lazy host effects and effect order at O0/O2. `plugin_borrowing_and_unique_reuse_are_visible_before_llvm_inlining`
asserts borrowing/root elimination and concat reuse before optimization. Provider code
is not forced alwaysinline. Forged allocation/host declarations, recursive/nested
providers, unknown exports and registry changes are rejected before execution.

## Measured boundary and limits

[Report and raw samples](../../benchmarks/reports/2026-10-04/V5_PLUGINS.md): 25 paired
repeats. Actual 16 MiB file hot loop: O0 **1919.354 → 1989.575 ms** (+4.185 ns/call),
O2 **433.580 → 396.647 ms** (-2.201 ns/call). Delta includes algorithm/instrumentation
and optimizer effects, not just dispatch. Root stores/frames stay unchanged and unique
replacement survives; provider introduces zero root traffic.

**Bulk equality regresses:** O0 **7.360 → 96.522 ms**, O2 **7.232 → 16.859 ms**.
The portable element algorithm replaces `memcmp`; retained lifetime proofs do not make
an algorithm equivalent in speed. Recommend an optimized compiler-visible lowering
before production adoption; opaque native plugin cost remains unmeasured.

Controls: scan16MiB **7.581 ms**, append1MiB **24.616 ms**, transform16MiB **35.357 ms**;
25 repeats, no clear regression against previous nonisolated sessions. Map curve and
known quadratic record-field append remain reported. Quota ceilings unchanged.

No runtime divergence or sanitizer defect was found. Review found an initial test used
unbounded defaults to claim budget coverage; explicit bounded eight-step success and
seven-step E008 now pass at O0/O2. Arbitrary native, allocating/effectful, recursive
providers and stable foreign ABI remain unsupported. No soundness guarantee was traded
for extensibility. [Worked third-party guide](../architecture/PLUGINS.md).
