# Verified-example exposure: validation and command evidence

Selection committed before inference: main 710331e, benchmarks 3883b98.
Runner committed next: main 76278ad, benchmarks 4cde3c2. No compiler/runtime/
profile/corpus sources changed. The experiment reused the frozen harness rather
than implementing another oracle or transport.

```text
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/application/fewshot/tests -v
exit 0; 3 tests: exact control preservation, fixed selection/hashes, validity accounting

benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/application/tests -v
exit 0; existing 6 frozen oracle/sandbox/accounting tests unchanged

benchmarks/paired/.venv/bin/python benchmarks/generation/application/fewshot/run.py --mock --output /tmp/nil-fewshot-exposure/stub-final
exit 0; 72 trials /162 attempts; STUB ONLY, never model evidence

benchmarks/paired/.venv/bin/python benchmarks/generation/application/fewshot/run.py --output /tmp/nil-fewshot-exposure/measured
exit 0; selected examples: 20 reference /40 native O0/O2 checks; all frozen fixture preflights pass
complete matrix: 72 trials /194 attempts, REAL MODEL USAGE; both exact digests

benchmarks/paired/.venv/bin/python benchmarks/generation/application/fewshot/run.py --report-only --output /tmp/nil-fewshot-exposure/measured
exit 0; complete-cell, source/prompt hash, usage and correctness audit

./scripts/ci.sh
exit 0; 368 tests, strict Clippy, zero warnings
```

The first stub attempt exposed a report assumption that both languages exist in
all arms. NIL-only arms now omit that within-arm comparison; the adapter reports
paired condition comparisons. This was corrected before any model request; no
real-model run was discarded, repeated or tuned. All controls and treatment used
the same machine/session and frozen seeds, budgets, repair feedback and oracles.
The control retains its pre-existing generic copy-file example; treatment adds the
fixed corpus examples, leaving the specification unchanged.

[Results and confidence limits](../../benchmarks/reports/2026-10-05/V5_FEWSHOT.md),
[verbatim command output](../../benchmarks/reports/2026-10-05/V5_FEWSHOT_RECEIPTS.txt),
[all attempts](../../benchmarks/reports/2026-10-05/V5_FEWSHOT_ATTEMPTS.jsonl),
[usage/stage/seed tables](../../benchmarks/reports/2026-10-05/V5_FEWSHOT_ANALYSIS.json).
Synthetic stub usage is separate from measured model tokens. Raw prompts,
responses and candidate sources are archived outside both repositories with the
checksum in the report. No models or executables are uploaded or pushed.
