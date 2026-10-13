#!/usr/bin/env python3
"""A cargo gate must not disturb gates that read build artifacts.

Round 28. Two defects of the same class were found in two rounds: the fuzz
binary was replaced mid-exec, and the depth audit linked
`target/release/*.rlib` while cargo was rewriting it. Both came from gates
reading the directory a cargo gate writes.

`gates.py` now freezes every artifact a gate reads into `OUT/bin` and gives the
cargo gates their own `CARGO_TARGET_DIR`. This starts a cargo build alongside
the two readers most sensitive to it and asserts all three succeed. Run it
directly; it is not part of the timed gate suite.
"""
import concurrent.futures
import json
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def run(command, env_extra=None):
    import os

    env = {**os.environ, **(env_extra or {})}
    done = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    return done.returncode, (done.stdout + done.stderr)[-400:]


def main():
    sys.path.insert(0, str(ROOT / "scripts"))
    import importlib.util

    spec = importlib.util.spec_from_file_location("gates", ROOT / "scripts/gates.py")
    gates = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(gates)

    with tempfile.TemporaryDirectory(prefix="nil-gate-isolation-") as directory:
        out = pathlib.Path(directory)
        frozen, artifacts = gates.freeze_artifacts(out)
        if "v5" not in artifacts or "libnil_hir.rlib" not in artifacts:
            print("build the release artifacts first", file=sys.stderr)
            return 2
        shared = {"NIL_BIN": str(frozen), "NIL_PREBUILT": "1"}

        jobs = {
            # The writer: a cargo build in its own target directory.
            "cargo": (
                ["cargo", "build", "--release", "--locked", "--offline", "-p", "nil"],
                {"CARGO_TARGET_DIR": str(ROOT / "target/gates-isolation")},
            ),
            # Reader one: links the rlibs that cargo would otherwise rewrite.
            "depth": (
                ["python3", "benchmarks/core-memory-study/audit_depth.py", "--out", str(out / "depth")],
                shared,
            ),
            # Reader two: execs the fuzz binary that was once replaced mid-run.
            "fuzz": (
                [str(frozen / "v5"), "--seed", "61001", "--scope", "io-faults",
                 "--out", str(out / "fuzz"), "--jobs", "2"],
                shared,
            ),
        }
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(jobs)) as pool:
            futures = {
                pool.submit(run, command, env): name
                for name, (command, env) in jobs.items()
            }
            results = {futures[f]: f.result() for f in concurrent.futures.as_completed(futures)}

        failed = {name: tail for name, (code, tail) in results.items() if code != 0}
        for name, (code, _) in sorted(results.items()):
            print(f"{name:6} exit {code}")
        if failed:
            for name, tail in failed.items():
                print(f"\n--- {name} ---\n{tail}", file=sys.stderr)
            print("\na cargo gate disturbed a reader", file=sys.stderr)
            return 1
        print(f"\nall three succeeded concurrently; {len(artifacts)} artifacts frozen")
        print(json.dumps({k: v[:12] for k, v in artifacts.items()}, indent=1))
        return 0


if __name__ == "__main__":
    sys.exit(main())
