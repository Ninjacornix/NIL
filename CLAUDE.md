@AGENTS.md

# Claude Code notes

`AGENTS.md` (imported above) holds the shared repository rules. This file adds only what
Claude Code needs beyond them: things learned while supervising implementation rounds.

## Roles

Implementation runs in rounds. A coding agent (Codex) receives one `/goal` per round. Claude
usually acts as overseer: it writes the goal, then independently re-runs every gate before
accepting the result. Never accept reported numbers without reproducing them. Local overseer
notes live in `docs/OVERSEER_*.md`; they are gitignored and may not exist on other machines.

## Product criteria

The maintainer judges every library, plugin or module change on two measured numbers only:

1. Call-site tokens (tiktoken cl100k and o200k), compared with an equivalent core intrinsic
   (`!sort(a)` is 4 tokens).
2. O2 runtime against the core implementation and C++.

How code is loaded (manifests, std embedding, plugin vs module) is an implementation detail.
Keeping an operation in core, when measurements show it cannot meet both numbers outside
core, is a valid outcome (ADR 035).

## Round discipline

- ADRs, forecasts and selection rules are committed before the implementation they govern.
  Check the ordering with `git log --reverse --name-only <prev>..HEAD`. Later ADR edits may
  only append measured results.
- Baselines are immutable in a measuring round: tasks, oracles, goldens and `.py`/`.cpp`
  baselines under `benchmarks/corpora/application-v5/`.
- Report unfavourable results first. A negative result with a published limit is a success.

## Audit commands

```sh
./scripts/ci.sh && ./scripts/ci.sh release      # test counts must not decrease
./scripts/sanitize.sh
./scripts/fuzz-v5.sh --seed <your own seed> --out <dir outside repo>   # >= 3 seeds
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output <f>
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py \
    --group original --output <f>                # must total exactly 610 checks
```

- The fuzzer selects families by `index % FAMILY_COUNT` (`tools/nil-fuzz/src/application.rs`).
  Omitting `--cases` runs every family once; fewer cases are rejected. Per-code counts are
  identical across seeds because seeds vary constants, not program structure.
- Re-measure performance yourself: at least 15 repeats (25 preferred), min/median/max. Machines
  differ by ~20% in absolute time, so compare ratios against controls, not raw milliseconds.

## Git and environment pitfalls

- `benchmarks/` is a submodule with its own remote (`NIL-benchmarks`). Push it before the main
  repo, or the main repo will point at a commit that does not exist on GitHub.
- PRs carrying ADR-ordered rounds merge with a merge commit, never squash. Squashing destroys
  the pre-registration trail and makes later PRs conflict. This is an exception to the squash
  rule in `AGENTS.md`.
- Local `master` can be very stale (from before the submodule existed) and fails to check out.
  Branch from `origin/master` instead, then run `git branch --unset-upstream` so a bare
  `git push` cannot target master.
- `gh` is not installed, so PRs are opened in the browser.
- The shell's `rtk` proxy filters git output and can hide merge commits in `git log`. Use
  `rtk proxy git ...` when exact output matters.
- Nothing is pushed without the maintainer's explicit approval.

## Reading NIL

The language reference is `docs/language/EXPR_V5.md`; ADRs are in `docs/adr/`. Programs are
positional: each line is a function labelled `a`, `b`, `c`… by position, and parameters are
`a`, `b`, `c`… within a function. `!name(...)` calls an intrinsic or the source std (`std/`),
`&c` passes function `c` as a static callback, and `^a(x)` invokes callback parameter `a`.
