# Round 15b — Claude Pro eval transport

## Outcome and limits

**No capable-model NIL/Python result, actual CLI preflight, authentication check or
measured spend was produced.** The maintainer explicitly reserved real-token
access and CLI execution for the overseer. The agent did not read the real token
file or invoke Claude. Offline tests used planted fake tokens and mocked CLI
subprocesses. They do not prove actual kernel sandbox enforcement or model access.
The earlier small-model NIL 0/24 versus Python 11/24 finding is unchanged.

Round 15's EPERM opening `/tmp/claude-501` was a **harness defect**, not evidence
that the machine lacks model access. The transport now sets CLAUDE_CODE_TMPDIR
inside a private root, beside an empty cwd and ephemeral home. No real-home config
exception, keychain read authority or `--bare` remains. Private token-file auth
passes CLAUDE_CODE_OAUTH_TOKEN only via the sandboxed CLI environment. Exact-secret
redaction happens before request metadata, stdout, stderr, diagnostics and campaign
artifacts are written. Mocks never open the real token file.

## Preregistration and accounting

[ADR 037's addendum](../adr/037.md) precedes implementation in both histories:

```text
main:       5ac1a85 subscription contract; 99da379 output-cap amendment
benchmarks: 4079e20 subscription plan; ea85ef0 output-cap amendment
code:       b41ac5d transport; 1732ddc resume/tests; main 342ec08 pins code
```

Caps are **8192 output/request and 16384/trial**, thinking included, identical
across arms. Three completed attempts, model/effort, split, tasks, prompts,
few-shot rule and oracles are unchanged. No live generation preceded this design
correction. The maximum output envelope becomes **2424832**, 10.69x old. That is
not predicted usage. No observed price/DEV usage supports a new dollar point
forecast. Worst-case request reservations USD443 exceed the unchanged USD50
notional cap; completing the matrix within it is not promised. Actual DEV mean/max
usage must be forecast and committed before held-out. Reports and forecasts label
subscription cost **notional**, not a subscription invoice. CLI enforcement can
overshoot after a call; report and stop if that happens.

Known-accounting usage windows pause outside scored attempt denominators. Incurred
tokens/cost remain included and reduce available budgets. No automatic retry;
explicit --resume preserves matrix position, completed attempts, conversations,
trial budgets and accounting probes. An atomic checkpoint marks each invocation
before it starts, then records completion. Unknown in-flight usage or unknown
rate-limit accounting prevents resume. Frozen hashes/revisions, CLI version and
journal checks plus a campaign lock prevent silent drift and concurrent replay.
Rate-paused invocations count toward the 443-invocation bound. SHA256 is accidental
corruption detection, not authentication of an adversarially edited journal.

## Validation output

Commands ran from the repository root; logs and raw mock artifacts are outside Git.

```text
./scripts/ci.sh
  exit 0; 441 tests passed; 0 warnings
./scripts/ci.sh release
  exit 0; 441 tests passed; 0 warnings
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/standing/tests -v
  Ran 39 tests; OK
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/application/tests -v
  Ran 6 tests; OK
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/generation/application/fewshot/tests -v
  Ran 3 tests; OK
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-gen-eval-15b/corpus.json
  exit 0; reference 563 + native 1126 + Python 563 + C++ 563 = 2815
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-gen-eval-15b/original.json
  exit 0; reference 120 + native 240 + Python 125 + C++ 125 = exactly 610
./scripts/gen-eval.sh --mock --phase held-out --output /tmp/nil-gen-eval-15b/mock-matrix
  exit 0; Status: complete attempts: 288
  SYNTHETIC: 144 trial cells, 48/48 supplied reference answers per arm
./scripts/gen-eval.sh --mock --mock-rate-limit-after 2 --output /tmp/nil-gen-eval-15b/cli-pause
  exit 75; Status: paused-rate-limit attempts: 2
./scripts/gen-eval.sh --resume /tmp/nil-gen-eval-15b/cli-pause
  exit 0; Status: complete attempts: 6
./scripts/gen-eval.sh --mock --phase dev --output /tmp/nil-gen-eval-15b/mock-dev
  exit 0; Status: complete attempts: 6
  All six attempt rows equal the resumed real-oracle mock rows.
git diff b47b9cf -- crates/ cli/ tools/ std/
  empty
git -C benchmarks diff 635046d -- corpora/ generation/application/
  empty
```

The 39 tests include private fake-token preflight through the actual transport
code with mocked CLI replies; sibling state writes leave cwd empty; group/other
permissions, symlinks, missing/empty files fail closed. A full mocked campaign
scans every artifact and published summary for planted OAuth/API tokens.
Resume tests cover repairs, authoritative pause charges, accounting probes,
unknown usage, crashes, frozen-plan/CLI drift, cap overruns and concurrent runs.

A test found unstable JSON key ordering between newly written and resumed attempt
logs; projections now serialize deterministically. Resume also retains rate-pause
output against the trial cap and blocks overruns, rather than resetting that budget.
No compiler bug was discovered or fixed. No compiler, runtime, std, corpus task,
solution, baseline, oracle, golden or Python dependency changed.

## Overseer commands

Provision the real subscription token at the default path, owned by the current
user and mode 0600; never paste it into chat, argv or a committed file. Use an
alternate path only via NIL_CLAUDE_OAUTH_TOKEN_FILE. Then run, on the maintainer's
machine, from this checkout, choosing fresh output paths:

```sh
./scripts/gen-eval.sh --preflight --output /tmp/nil-gen-pro/preflight
./scripts/gen-eval.sh --phase dev --output /tmp/nil-gen-pro/dev
# If DEV paused, resume it BEFORE forecasting:
./scripts/gen-eval.sh --resume /tmp/nil-gen-pro/dev
benchmarks/paired/.venv/bin/python benchmarks/generation/standing/forecast.py --dev /tmp/nil-gen-pro/dev
git -C benchmarks add generation/standing/cost-forecast.json
git -C benchmarks commit -m 'docs(eval): forecast notional held-out cost'
git add benchmarks
git commit -m 'docs(eval): pin observed generation forecast'
./scripts/gen-eval.sh --phase held-out --prior /tmp/nil-gen-pro/dev --output /tmp/nil-gen-pro/held-out
# Only if held-out pauses; keep repository revisions unchanged while paused:
./scripts/gen-eval.sh --resume /tmp/nil-gen-pro/held-out
```

Preflight denies all networking and makes no generation request. Passing it only
proves startup isolation; the first DEV request still checks actual model/usage.
Resume is only for a paused campaign, not completed/blocked campaigns. Keep the
USD50 notional cap; publish partial results if reached. Do not substitute models
or adjust the frozen matrix after results. Full details are in
[the transport protocol](../../benchmarks/generation/standing/README.md).

## Evidence archive

`/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-subscription-eval-2026-10-06.tar.gz`

SHA256: `e2dfa09dad7954580a648c2727e41a206a95a5018603913dce49afc5354b1b50`.
Contains final gate logs, gates.json, external mock requests/responses/checkpoints
and summaries; excludes binaries and superseded test logs. Credential-pattern
scan of tracked files found zero matches. No real credentials were accessed.
Historical Round 15 reports remain unchanged; generated Round 15b summaries are
archived externally. All commits local; nothing pushed. The pre-existing untracked
OVERSEER_HANDOFF.md remains untouched.
