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
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
JOB_BUDGET = 7
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
        result = run(["./scripts/ci.sh"] + ([mode] if mode else []))
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
        result = run(["./scripts/fuzz-v5.sh", "--seed", str(seed), "--out", str(out), "--jobs", str(jobs)])
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


def changed_paths(base):
    diff = run(["git", "diff", "--name-only", base])
    return diff["stdout"].split() if diff["exit"] == 0 else []


def plan(mode, out, base, seeds):
    """Gate name -> (callable, serial).

    `serial` means the gate must not share the machine, which now covers only
    gates that measure time. The cohorts used to qualify too, because the
    harness enforced a 120 s *wall* budget per check and a concurrent run
    reported 1,067/1,070 and 1,069/1,070 purely from load; that budget is now
    CPU seconds, so they are load independent and run concurrently.
    """
    gates = {}
    if mode == "fast":
        gates["ci"] = (gate_ci(None), False)
        gates[f"fuzz-{seeds[0]}"] = (gate_fuzz(seeds[0], out / f"fuzz-{seeds[0]}", 5), False)
        gates["corpus"] = (gate_corpus(out / "corpus.json"), False)
        gates["cohort-tuned"] = (gate_cohort("tuned", None, out / "cohort-tuned.json"), False)
        gates["cohort-natural"] = (
            gate_cohort("natural", "benchmarks/core-generalize-study/natural", out / "cohort-natural.json"),
            False,
        )
        touched = changed_paths(base)
        if any(path.startswith(MEMORY_PATHS) for path in touched):
            gates["sanitize"] = (gate_sanitize(), False)
            gates["depth"] = (gate_depth(out / "depth"), False)
    else:
        gates["ci"] = (gate_ci(None), False)
        gates["ci-release"] = (gate_ci("release"), False)
        gates["sanitize"] = (gate_sanitize(), False)
        for seed in seeds:
            gates[f"fuzz-{seed}"] = (gate_fuzz(seed, out / f"fuzz-{seed}", 5), False)
        gates["corpus"] = (
            gate_corpus(out / "corpus.json", out / "corpus-original.json"),
            False,
        )
        gates["cohort-tuned"] = (gate_cohort("tuned", None, out / "cohort-tuned.json"), False)
        gates["cohort-natural"] = (
            gate_cohort("natural", "benchmarks/core-generalize-study/natural", out / "cohort-natural.json"),
            False,
        )
        gates["depth"] = (gate_depth(out / "depth"), True)
    return gates


def execute(gates, budget):
    """Concurrent, except serial gates never share the machine."""
    results = {}
    timed = {name: body for name, (body, serial) in gates.items() if serial}
    untimed = {name: body for name, (body, serial) in gates.items() if not serial}
    with concurrent.futures.ThreadPoolExecutor(max_workers=budget) as pool:
        futures = {pool.submit(body): name for name, body in untimed.items()}
        for future in concurrent.futures.as_completed(futures):
            results[futures[future]] = future.result()
    for name, body in timed.items():
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


def main():
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
    arguments = parser.parse_args()

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
        for name, counts in expected.get("gates", {}).items():
            actual = receipt["gates"].get(name, {}).get("counts", {})
            for key, want in counts.items():
                got = actual.get(key)
                if isinstance(want, int) and isinstance(got, int) and got < want:
                    regressed.append(f"{name}.{key}: {got} < {want}")

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
