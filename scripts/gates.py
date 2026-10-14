#!/usr/bin/env python3
"""Round 27 P7: one entry point for the gate suite, with a receipt.

`fast` is the pre-commit sweep and aims to stay under ten minutes. `full` is the
evidence run. Both write a receipt naming exactly what ran: the revisions, the
toolchain, the SHA-256 of every binary measured, and each gate's command, exit
status, wall time and extracted counts, with the fuzz summaries kept verbatim.

Gates run concurrently under a job budget, except that nothing runs beside a
gate that measures time.
"""
import argparse
import concurrent.futures
import threading
import hashlib
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
# Effective-core budget, not a gate count: several gates are themselves
# parallel, and scheduling by gate count oversubscribed the machine badly
# enough to make the fuzz harness and the cohorts report false timeouts.
# Slightly above the core count on purpose: gates are subprocess bound and wait
# on clang and child processes, so measured wall time was 1,355.7 s at cores+2
# against 1,681.8 s at cores-2 on an eight-core machine, both fully green.
JOB_BUDGET = (os.cpu_count() or 4) + 2
FUZZ_JOBS = 5
CORPUS_JOBS = 6
FUZZ_SEEDS = (61001, 83003)
# Paths whose change makes the slow memory gates relevant to a fast run.
MEMORY_PATHS = ("crates/nil-llvm/", "crates/nil-hir/", "plugins/")


def run(command, cwd=ROOT, env=None):
    merged = {**os.environ, **(env or {})}
    start = time.perf_counter()
    done = subprocess.run(
        command, cwd=cwd, env=merged, capture_output=True, text=True, shell=isinstance(command, str)
    )
    return {
        "command": command if isinstance(command, str) else " ".join(map(str, command)),
        "exit": done.returncode,
        "seconds": round(time.perf_counter() - start, 2),
        "stdout": done.stdout,
        "stderr": done.stderr,
    }


def sha256_file(path):
    path = pathlib.Path(path)
    return sha256_file.cache.setdefault(
        str(path), hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
    )


sha256_file.cache = {}


def tests_passed(text):
    return sum(int(m.group(1)) for m in re.finditer(r"^test result: ok\. (\d+) passed", text, re.M))


def sanitizer_findings(text):
    return len(re.findall(r"ERROR: AddressSanitizer|runtime error:|LeakSanitizer", text))


def totals_of(text):
    """Last TOTALS line of a verify.py run, as a dict."""
    found = [line for line in text.splitlines() if line.startswith("TOTALS ")]
    return json.loads(found[-1][len("TOTALS ") :]) if found else None


def gate_ci(mode):
    def body():
        # Its own target directory, so a cargo gate never writes a file another
        # gate reads, and the two cargo gates never fight each other.
        label = mode or "debug"
        result = run(
            ["./scripts/ci.sh"] + ([mode] if mode else []),
            env={"CARGO_TARGET_DIR": str(ROOT / f"target/gates-{label}")},
        )
        result["counts"] = {"tests": tests_passed(result["stdout"])}
        result["warnings"] = len(re.findall(r"^warning", result["stdout"], re.M))
        return result

    return body


def gate_sanitize():
    def body():
        result = run(["./scripts/sanitize.sh"])
        result["counts"] = {"tests": tests_passed(result["stdout"])}
        result["findings"] = sanitizer_findings(result["stdout"] + result["stderr"])
        return result

    return body


def gate_fuzz(seed, out, jobs):
    def body():
        # scripts/fuzz-v5.sh rebuilds the binary before exec'ing it, so two
        # concurrent fuzz gates can replace target/release/v5 while the other is
        # running it. Under NIL_PREBUILT the wrapper has already built it, so
        # exec it directly and never rebuild inside a gate.
        prebuilt = pathlib.Path(os.environ["NIL_BIN"]) / "v5"
        if os.environ.get("NIL_PREBUILT") == "1":
            if not prebuilt.is_file():
                return {
                    "command": str(prebuilt),
                    "exit": 2,
                    "seconds": 0.0,
                    "stdout": "",
                    "stderr": f"NIL_PREBUILT=1 but {prebuilt} is missing",
                    "counts": {},
                }
            command = [str(prebuilt)]
        else:
            command = ["./scripts/fuzz-v5.sh"]
        result = run(command + ["--seed", str(seed), "--out", str(out), "--jobs", str(jobs)])
        tail = result["stdout"].strip().splitlines()
        # Kept verbatim: the summary is the evidence, not a derived count.
        result["summary"] = tail[-1] if tail else ""
        try:
            parsed = json.loads(result["summary"])
            result["counts"] = {
                "programs": parsed["programs"],
                "non_resource_divergences": parsed["non_resource_divergences"],
                "rule_3_failures": parsed["rule_3_failures"],
            }
        except (ValueError, KeyError):
            result["counts"] = {}
        return result

    return body


def gate_corpus(out, original_out=None, jobs=6):
    """One run covers both groups: the original report is derived (P3)."""

    def body():
        command = [
            "benchmarks/paired/.venv/bin/python",
            "benchmarks/corpora/application-v5/verify.py",
            "--output",
            str(out),
            "--jobs",
            str(jobs),
        ]
        if original_out:
            command += ["--original-output", str(original_out)]
        result = run(command)
        counts = {}
        for prefix, label in (("TOTALS ", "checks"), ("TOTALS-ORIGINAL ", "original_checks")):
            lines = [line for line in result["stdout"].splitlines() if line.startswith(prefix)]
            if not lines:
                continue
            totals = json.loads(lines[-1][len(prefix) :])
            counts[label] = sum(
                totals.get(key, 0)
                for key in ("reference_checks", "native_checks", "python_checks", "cpp_checks")
            )
            if label == "checks":
                counts["inconclusive"] = totals.get("inconclusive_checks")
        result["counts"] = counts
        return result

    return body


def gate_cohort(name, candidate, out):
    def body():
        command = [
            "python3",
            "benchmarks/corpora/neetcode150/verify.py",
            "--verified-only",
            "--output",
            str(out),
        ]
        if candidate:
            command += ["--candidate-dir", str(ROOT / candidate)]
        result = run(command)
        totals = totals_of(result["stdout"]) or {}
        result["counts"] = {
            "checks": totals.get("checks"),
            "passed": totals.get("passed_checks"),
        }
        return result

    return body


def gate_depth(out):
    def body():
        result = run(["python3", "benchmarks/core-memory-study/audit_depth.py", "--out", str(out)])
        match = re.search(r"(\d+)/(\d+) nondivergent", result["stdout"])
        result["counts"] = {"nondivergent": match.group(0) if match else None}
        return result

    return body


def freeze_artifacts(out):
    """Copies every artifact a gate reads into OUT/bin.

    Round 28: this kills the race class rather than instances. Gates read only
    from this frozen copy, so no gate can read a file another gate's cargo is
    rewriting — the defect that replaced the fuzz binary mid-exec and made the
    depth audit link a half-written rlib.
    """
    source = ROOT / "target/release"
    frozen = out / "bin"
    (frozen / "examples").mkdir(parents=True, exist_ok=True)
    (frozen / "deps").mkdir(parents=True, exist_ok=True)
    wanted = [("nil", ""), ("v5", "")]
    wanted += [(name, "examples") for name in ("core_check", "application_check")]
    digests = {}
    for name, sub in wanted:
        origin = source / sub / name if sub else source / name
        if not origin.is_file():
            continue
        target = frozen / sub / name if sub else frozen / name
        shutil.copy2(origin, target)
        digests[f"{sub}/{name}" if sub else name] = sha256_file(target)
    # The unhashed target/release/lib<crate>.rlib is not reliably refreshed:
    # building a single package writes only deps/lib<crate>-<hash>.rlib, so the
    # top-level copy can lag by hours and rustc then rejects it against freshly
    # frozen dependencies. Freeze what cargo actually produced, newest first.
    for crate in ("nil_compiler", "nil_hir", "nil_llvm"):
        candidates = sorted(
            (source / "deps").glob(f"lib{crate}-*.rlib"),
            key=lambda entry: entry.stat().st_mtime,
            reverse=True,
        )
        fallback = source / f"lib{crate}.rlib"
        if not candidates and fallback.is_file():
            candidates = [fallback]
        if not candidates:
            continue
        target = frozen / f"lib{crate}.rlib"
        shutil.copy2(candidates[0], target)
        digests[f"lib{crate}.rlib"] = sha256_file(target)
    # audit_depth links against the dependency rlibs too.
    if (source / "deps").is_dir():
        for entry in (source / "deps").iterdir():
            if entry.suffix in (".rlib", ".rmeta") and entry.is_file():
                shutil.copy2(entry, frozen / "deps" / entry.name)
    return frozen, digests


def changed_paths(base):
    diff = run(["git", "diff", "--name-only", base])
    return diff["stdout"].split() if diff["exit"] == 0 else []


def plan(mode, out, base, seeds):
    """Gate name -> (callable, effective cores, must run alone).

    The third element means the gate must run alone. That covers gates which
    measure time, and the two cargo gates: they write `target/`, so running
    them beside each other or beside a gate that reads the prebuilt artifacts
    is a race. One full run had the depth audit fail in 0.2 s because `rustc`
    linked `target/release/*.rlib` while cargo was rewriting it — the same class
    of race as the fuzz binary being replaced mid-exec.

    The cohorts no longer need it: their per-check budget is CPU seconds rather
    than wall seconds, so they are load independent.
    """
    gates = {}
    if mode == "fast":
        gates["ci"] = (gate_ci(None), 4, True)
        gates[f"fuzz-{seeds[0]}"] = (gate_fuzz(seeds[0], out / f"fuzz-{seeds[0]}", FUZZ_JOBS), FUZZ_JOBS, False)
        gates["corpus"] = (gate_corpus(out / "corpus.json", jobs=CORPUS_JOBS), CORPUS_JOBS, False)
        gates["cohort-tuned"] = (gate_cohort("tuned", None, out / "cohort-tuned.json"), 1, False)
        gates["cohort-natural"] = (
            gate_cohort("natural", "benchmarks/core-generalize-study/natural", out / "cohort-natural.json"),
            1,
            False,
        )
        touched = changed_paths(base)
        if any(path.startswith(MEMORY_PATHS) for path in touched):
            gates["sanitize"] = (gate_sanitize(), 4, False)
            gates["depth"] = (gate_depth(out / "depth"), 1, False)
    else:
        gates["ci"] = (gate_ci(None), 4, True)
        gates["ci-release"] = (gate_ci("release"), 4, True)
        gates["sanitize"] = (gate_sanitize(), 4, False)
        for seed in seeds:
            gates[f"fuzz-{seed}"] = (gate_fuzz(seed, out / f"fuzz-{seed}", FUZZ_JOBS), FUZZ_JOBS, False)
        gates["corpus"] = (
            gate_corpus(out / "corpus.json", out / "corpus-original.json", CORPUS_JOBS),
            CORPUS_JOBS,
            False,
        )
        gates["cohort-tuned"] = (gate_cohort("tuned", None, out / "cohort-tuned.json"), 1, False)
        gates["cohort-natural"] = (
            gate_cohort("natural", "benchmarks/core-generalize-study/natural", out / "cohort-natural.json"),
            1,
            False,
        )
        gates["depth"] = (gate_depth(out / "depth"), 1, False)
    return gates


def execute(gates, budget):
    """Concurrent within an effective-core budget.

    Each gate declares the cores it will actually use, because several are
    internally parallel. Scheduling by gate count instead oversubscribed the
    machine about twentyfold, which made the fuzz harness report a native
    timeout and a cohort report a reference timeout — both false.
    """
    results = {}
    serial = [(name, body) for name, (body, weight, alone) in gates.items() if alone]
    shared = [(name, body, weight) for name, (body, weight, alone) in gates.items() if not alone]
    # Heaviest first, so a wide gate never waits behind a queue of narrow ones.
    shared.sort(key=lambda item: -item[2])
    available = threading.Semaphore(budget)
    # Admission is serialized so a gate never holds a partial allocation while
    # waiting for the rest. Without this, two wide gates each held some permits
    # and waited for each other: a full run measured 10,481 s wall against
    # 2,622 s of gate time, almost all of it starvation.
    admission = threading.Lock()

    def admit(name, body, weight):
        taken = min(weight, budget)
        with admission:
            for _ in range(taken):
                available.acquire()
        try:
            return name, body()
        finally:
            for _ in range(taken):
                available.release()

    with concurrent.futures.ThreadPoolExecutor(max_workers=len(shared) or 1) as pool:
        futures = [pool.submit(admit, name, body, weight) for name, body, weight in shared]
        for future in concurrent.futures.as_completed(futures):
            name, result = future.result()
            results[name] = result
    for name, body in serial:
        results[name] = body()
    return results


def provenance():
    head = run(["git", "rev-parse", "HEAD"])["stdout"].strip()
    dirty = run(["git", "status", "--porcelain"])["stdout"].strip() != ""
    bench = run(["git", "rev-parse", "HEAD"], cwd=ROOT / "benchmarks")["stdout"].strip()
    toolchain = run(["rustc", "--version"])["stdout"].strip()
    clang = run([os.environ.get("NIL_CLANG", "clang"), "--version"])["stdout"].splitlines()
    binaries = {
        name: sha256_file(ROOT / "target/release" / name)
        for name in ("nil", "v5")
        if (ROOT / "target/release" / name).is_file()
    }
    helper = ROOT / "target/release/examples/core_check"
    if helper.is_file():
        binaries["core_check"] = sha256_file(helper)
    return {
        "head": head,
        "dirty": dirty,
        "benchmarks_revision": bench,
        "toolchain": toolchain,
        "clang": clang[0] if clang else None,
        "binaries": binaries,
    }


def count_regressions(expected, actual):
    regressed = []
    for name, counts in expected.items():
        actual_counts = actual.get(name, {}).get("counts", {})
        for key, want in counts.items():
            got = actual_counts.get(key)
            if isinstance(want, int) and (
                not isinstance(got, int) or got < want
            ):
                regressed.append(f"{name}.{key}: {got} < {want}")
    return regressed


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("fast", "full"))
    parser.add_argument("--out", required=True)
    parser.add_argument("--base", default="HEAD", help="revision to diff against for fast-mode relevance")
    parser.add_argument("--expect", help="counts.json that the run must not fall below")
    parser.add_argument("--jobs", type=int, default=JOB_BUDGET)
    parser.add_argument(
        "--seeds",
        default=",".join(str(seed) for seed in FUZZ_SEEDS),
        help="comma separated fuzz seeds; fast uses the first. Independent seeds "
        "let a reviewer re-run the evidence without reusing ours.",
    )
    arguments = parser.parse_args(argv)
    if arguments.jobs <= 0:
        parser.error("--jobs must be a positive integer")

    out = pathlib.Path(arguments.out).expanduser().resolve()
    # Artifacts never land in the repository: a gate run must not dirty the tree
    # it is judging.
    if out == ROOT or ROOT in out.parents:
        print(f"--out must be outside {ROOT}", file=sys.stderr)
        return 2
    out.mkdir(parents=True, exist_ok=True)

    try:
        seeds = [int(part) for part in arguments.seeds.split(",") if part.strip()]
    except ValueError:
        print("--seeds must be comma separated integers", file=sys.stderr)
        return 2
    if not seeds:
        print("--seeds needs at least one seed", file=sys.stderr)
        return 2

    frozen, artifacts = freeze_artifacts(out)
    # Every non-cargo gate reads only the frozen copy.
    os.environ["NIL_BIN"] = str(frozen)

    started = time.time()
    gates = plan(arguments.mode, out, arguments.base, seeds)
    results = execute(gates, arguments.jobs)
    receipt = {
        "schema": 1,
        "mode": arguments.mode,
        "started": int(started),
        "wall_seconds": round(time.time() - started, 2),
        "job_budget": arguments.jobs,
        "seeds": seeds,
        "provenance": provenance(),
        "frozen_artifacts": artifacts,
        "gates": {
            name: {key: value for key, value in result.items() if key not in ("stdout", "stderr")}
            for name, result in results.items()
        },
    }
    for name, result in results.items():
        (out / f"{name}.log").write_text(result["stdout"] + result["stderr"])
    receipt["report_sha256"] = {
        path.name: sha256_file(path) for path in sorted(out.glob("*.json")) if path.name != "receipt.json"
    }
    (out / "receipt.json").write_text(json.dumps(receipt, indent=1))

    failed = [name for name, result in results.items() if result["exit"] != 0]
    regressed = []
    if arguments.expect:
        expected = json.loads(pathlib.Path(arguments.expect).read_text())
        regressed = count_regressions(expected.get("gates", {}), receipt["gates"])

    width = max(len(name) for name in results)
    for name in sorted(results):
        result = results[name]
        print(
            f"{name:<{width}}  exit {result['exit']:>3}  {result['seconds']:>8.1f}s  "
            f"{result.get('counts', {})}"
        )
    print(f"\n{arguments.mode}: {receipt['wall_seconds']:.1f}s wall, receipt at {out / 'receipt.json'}")
    for item in regressed:
        print(f"REGRESSED {item}", file=sys.stderr)
    if failed:
        print(f"FAILED gates: {', '.join(sorted(failed))}", file=sys.stderr)
    return 1 if failed or regressed else 0


if __name__ == "__main__":
    sys.exit(main())
