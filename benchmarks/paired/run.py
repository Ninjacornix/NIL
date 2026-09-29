"""Paired source-token and interpreter runtime benchmark for NIL and Python."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

import tiktoken


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
ENCODING = "cl100k_base"


def source_measure(path: Path, encoding: tiktoken.Encoding) -> dict:
    data = path.read_bytes()
    source = data.decode("utf-8")
    return {
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": len(data),
        "characters": len(source),
        "tokens": len(encoding.encode(source, allowed_special=set(), disallowed_special=())),
    }


def load_program(path: Path):
    spec = importlib.util.spec_from_file_location(f"nil_bench_{path.stem}", path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.program


def rust_call(binary: Path, source: Path, label: int, expected: int,
              warmup: int, iterations: int, repeats: int, args: list[int]) -> list[float]:
    command = [str(binary), str(source), str(label), str(expected), str(warmup),
               str(iterations), str(repeats), *(str(arg) for arg in args)]
    result = subprocess.run(command, check=True, capture_output=True, text=True,
                            timeout=120)
    samples = json.loads(result.stdout)["ns_per_call"]
    if len(samples) != repeats or not all(isinstance(x, (int, float)) and x >= 0
                                           for x in samples):
        raise ValueError(f"invalid NIL timing samples for {source}")
    return samples


def python_call(program, expected: int, warmup: int, iterations: int,
                repeats: int, args: list[int]) -> list[float]:
    for _ in range(warmup):
        if program(*args) != expected:
            raise ValueError("Python warmup result differs from expected")
    samples = []
    for _ in range(repeats):
        start = time.perf_counter_ns()
        for _ in range(iterations):
            if program(*args) != expected:
                raise ValueError("Python timed result differs from expected")
        samples.append((time.perf_counter_ns() - start) / iterations)
    return samples


def comparison(nil_value: float, python_value: float) -> dict:
    return {
        "nil_over_python": nil_value / python_value,
        "nil_better": nil_value < python_value,
    }


def validate_cases(cases: list[dict]) -> None:
    ids = set()
    for case in cases:
        if case["id"] in ids:
            raise ValueError(f"duplicate case {case['id']}")
        ids.add(case["id"])
        if not case["checks"]:
            raise ValueError(f"case {case['id']} has no checks")
        for check in case["checks"]:
            if not isinstance(check["expected"], int) or not all(
                isinstance(arg, int) for arg in check["args"]
            ):
                raise ValueError(f"case {case['id']} has noninteger input or output")


def benchmark(binary: Path, iterations: int, warmup: int, repeats: int) -> dict:
    manifest = json.loads((HERE / "cases.json").read_text(encoding="utf-8"))
    if manifest["schema"] != 1:
        raise ValueError("unsupported corpus schema")
    validate_cases(manifest["cases"])
    encoding = tiktoken.get_encoding(ENCODING)
    results = []
    for case in manifest["cases"]:
        nil_file = HERE / case["nil"]
        python_file = HERE / case["python"]
        program = load_program(python_file)
        for check in case["checks"]:
            args, expected = check["args"], check["expected"]
            if program(*args) != expected:
                raise ValueError(f"Python correctness failed: {case['id']} {args}")
            rust_call(binary, nil_file, case["function"], expected, 0, 1, 1, args)
        timed = case["checks"][0]
        nil_samples = rust_call(binary, nil_file, case["function"], timed["expected"],
                                warmup, iterations, repeats, timed["args"])
        python_samples = python_call(program, timed["expected"], warmup, iterations,
                                     repeats, timed["args"])
        nil_ns = statistics.median(nil_samples)
        python_ns = statistics.median(python_samples)
        nil_source = source_measure(nil_file, encoding)
        python_source = source_measure(python_file, encoding)
        results.append({
            "id": case["id"], "function": case["function"],
            "checks": case["checks"], "timed_args": timed["args"],
            "nil": {"file": case["nil"], "source": nil_source,
                    "runtime_ns_per_call": nil_samples, "median_ns_per_call": nil_ns},
            "python": {"file": case["python"], "source": python_source,
                       "runtime_ns_per_call": python_samples,
                       "median_ns_per_call": python_ns},
            "tokens": comparison(nil_source["tokens"], python_source["tokens"]),
            "speed": comparison(nil_ns, python_ns),
        })
    return {
        "schema": 1,
        "method": "whole source; cl100k_base raw tokens; already compiled NIL interpreter vs loaded Python function; per-call medians",
        "tokenizer": {"encoding": ENCODING, "package": "tiktoken",
                      "version": tiktoken.__version__,
                      "special_tokens": "ordinary text"},
        "environment": {
            "python": platform.python_version(),
            "platform": platform.platform(),
            "rustc": subprocess.run(["rustc", "--version"], cwd=ROOT, check=True,
                                    capture_output=True, text=True).stdout.strip(),
            "git_revision": subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                           check=True, capture_output=True,
                                           text=True).stdout.strip(),
            "git_dirty": bool(subprocess.run(["git", "status", "--porcelain"], cwd=ROOT,
                                             check=True, capture_output=True,
                                             text=True).stdout.strip()),
        },
        "settings": {"warmup": warmup, "iterations": iterations, "repeats": repeats},
        "cases": results,
        "summary": {
            "cases": len(results),
            "nil_fewer_tokens": sum(case["tokens"]["nil_better"] for case in results),
            "nil_faster": sum(case["speed"]["nil_better"] for case in results),
            "nil_total_tokens": sum(case["nil"]["source"]["tokens"] for case in results),
            "python_total_tokens": sum(case["python"]["source"]["tokens"] for case in results),
        },
    }


def table(report: dict) -> str:
    lines = ["Case         Tokens NIL/Python  NIL/Python token ratio  Runtime NIL/Python ns  NIL/Python speed ratio",
             "------------ ------------------ ----------------------- ---------------------- ----------------------"]
    for case in report["cases"]:
        lines.append(
            f"{case['id']:<12} "
            f"{case['nil']['source']['tokens']:>5}/{case['python']['source']['tokens']:<12} "
            f"{case['tokens']['nil_over_python']:>9.3f}               "
            f"{case['nil']['median_ns_per_call']:>8.1f}/{case['python']['median_ns_per_call']:<8.1f} "
            f"{case['speed']['nil_over_python']:>9.3f}"
        )
    summary = report["summary"]
    lines.append(f"NIL wins: {summary['nil_fewer_tokens']}/{summary['cases']} token cases; "
                 f"{summary['nil_faster']}/{summary['cases']} speed cases. "
                 "Ratio < 1 means NIL uses fewer tokens or runs faster.")
    lines.append("Speed measures NIL reference interpreter against CPython function calls; "
                 "small programs include call/measurement overhead.")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--warmup", type=int, default=1_000)
    parser.add_argument("--iterations", type=int, default=10_000)
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--format", choices=("table", "json"), default="table")
    args = parser.parse_args()
    if not 0 <= args.warmup <= 10_000_000 or not 1 <= args.iterations <= 10_000_000 \
            or not 1 <= args.repeats <= 100:
        parser.error("warmup must be 0..10000000, iterations 1..10000000, repeats 1..100")
    suffix = ".exe" if sys.platform == "win32" else ""
    binary = ROOT / "target" / "release" / "examples" / f"paired_runtime{suffix}"
    subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "-p",
                    "nil-compiler", "--example", "paired_runtime"], cwd=ROOT, check=True)
    report = benchmark(binary, args.iterations, args.warmup, args.repeats)
    print(json.dumps(report, indent=2) if args.format == "json" else table(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
