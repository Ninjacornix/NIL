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
import tempfile
import time
import urllib.request
from pathlib import Path

import tiktoken
import tokenizers


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
QWEN_REVISION = "098145f275b49d4517571a8c5d1e7896f68797d8"
QWEN_SHA256 = "c0382117ea329cdf097041132f6d735924b697924d6f6fc3945713e96ce87539"
QWEN_URL = ("https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B-Instruct/resolve/"
            f"{QWEN_REVISION}/tokenizer.json")
QWEN_ASSET = HERE / ".cache" / "qwen2.5-coder-tokenizer.json"


def verified_qwen_asset(path: Path = QWEN_ASSET) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        temporary_path = None
        try:
            with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as temporary:
                temporary_path = Path(temporary.name)
                with urllib.request.urlopen(QWEN_URL, timeout=60) as response:
                    while chunk := response.read(1024 * 1024):
                        temporary.write(chunk)
            if hashlib.sha256(temporary_path.read_bytes()).hexdigest() != QWEN_SHA256:
                raise ValueError("Qwen tokenizer asset SHA-256 mismatch")
            temporary_path.replace(path)
        finally:
            if temporary_path is not None:
                temporary_path.unlink(missing_ok=True)
    if hashlib.sha256(path.read_bytes()).hexdigest() != QWEN_SHA256:
        raise ValueError(f"Qwen tokenizer asset SHA-256 mismatch: {path}")
    return path


def load_tokenizers() -> tuple[dict, list[dict]]:
    cl100k = tiktoken.get_encoding("cl100k_base")
    qwen = tokenizers.Tokenizer.from_file(str(verified_qwen_asset()))
    counters = {
        "cl100k_base": lambda source: len(cl100k.encode(
            source, allowed_special=set(), disallowed_special=())),
        "qwen2.5_coder": lambda source: len(qwen.encode(
            source, add_special_tokens=False).ids),
    }
    metadata = [
        {"id": "cl100k_base", "package": "tiktoken", "version": tiktoken.__version__,
         "encoding": "cl100k_base", "special_tokens": "ordinary text"},
        {"id": "qwen2.5_coder", "package": "tokenizers", "version": tokenizers.__version__,
         "model": "Qwen/Qwen2.5-Coder-1.5B-Instruct", "revision": QWEN_REVISION,
         "asset_sha256": QWEN_SHA256, "special_tokens": "none added"},
    ]
    return counters, metadata


def source_measure(path: Path, counters: dict) -> dict:
    data = path.read_bytes()
    source = data.decode("utf-8")
    return {
        "sha256": hashlib.sha256(data).hexdigest(),
        "bytes": len(data),
        "characters": len(source),
        "tokens": {name: count(source) for name, count in counters.items()},
    }


def load_program(path: Path):
    spec = importlib.util.spec_from_file_location(f"nil_bench_{path.stem}", path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.program


def rust_call(binary: Path, source: Path, profile: str, label: int, expected: int,
              warmup: int, iterations: int, repeats: int, args: list[int]) -> list[float]:
    command = [str(binary), "--profile", profile, str(source), str(label), str(expected), str(warmup),
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


def benchmark(binary: Path, iterations: int, warmup: int, repeats: int,
              manifest_path: Path) -> dict:
    manifest_bytes = manifest_path.read_bytes()
    manifest = json.loads(manifest_bytes)
    if manifest["schema"] != 1:
        raise ValueError("unsupported corpus schema")
    validate_cases(manifest["cases"])
    counters, tokenizer_metadata = load_tokenizers()
    results = []
    for case in manifest["cases"]:
        nil_file = HERE / case["nil"]
        python_file = HERE / case["python"]
        program = load_program(python_file)
        for check in case["checks"]:
            args, expected = check["args"], check["expected"]
            if program(*args) != expected:
                raise ValueError(f"Python correctness failed: {case['id']} {args}")
            rust_call(binary, nil_file, case.get("profile", "lines-v0"),
                      case["function"], expected, 0, 1, 1, args)
        timed = case["checks"][0]
        nil_samples = rust_call(binary, nil_file, case.get("profile", "lines-v0"),
                                case["function"], timed["expected"],
                                warmup, iterations, repeats, timed["args"])
        python_samples = python_call(program, timed["expected"], warmup, iterations,
                                     repeats, timed["args"])
        nil_ns = statistics.median(nil_samples)
        python_ns = statistics.median(python_samples)
        nil_source = source_measure(nil_file, counters)
        python_source = source_measure(python_file, counters)
        results.append({
            "id": case["id"], "function": case["function"],
            "profile": case.get("profile", "lines-v0"),
            "checks": case["checks"], "timed_args": timed["args"],
            "nil": {"file": case["nil"], "source": nil_source,
                    "runtime_ns_per_call": nil_samples, "median_ns_per_call": nil_ns},
            "python": {"file": case["python"], "source": python_source,
                       "runtime_ns_per_call": python_samples,
                       "median_ns_per_call": python_ns},
            "tokens": {name: comparison(nil_source["tokens"][name],
                                        python_source["tokens"][name]) for name in counters},
            "speed": comparison(nil_ns, python_ns),
        })
    return {
        "schema": 2,
        "manifest": {"path": str(manifest_path),
                     "sha256": hashlib.sha256(manifest_bytes).hexdigest()},
        "method": "whole source, raw tokens without framing; already compiled NIL interpreter vs loaded Python function; per-call medians",
        "tokenizers": tokenizer_metadata,
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
            "nil_faster": sum(case["speed"]["nil_better"] for case in results),
            "tokens": {name: {
                "nil_fewer": sum(case["tokens"][name]["nil_better"] for case in results),
                "nil_total": sum(case["nil"]["source"]["tokens"][name]
                                 for case in results),
                "python_total": sum(case["python"]["source"]["tokens"][name]
                                    for case in results),
            } for name in counters},
        },
    }


def table(report: dict) -> str:
    lines = ["Source tokens (whole file; ratio < 1 favors NIL)",
             "Tokenizer       Case          NIL  Python  NIL/Python"]
    for name in report["summary"]["tokens"]:
        for case in report["cases"]:
            lines.append(f"{name:<15} {case['id']:<12} "
                         f"{case['nil']['source']['tokens'][name]:>4} "
                         f"{case['python']['source']['tokens'][name]:>7} "
                         f"{case['tokens'][name]['nil_over_python']:>11.3f}")
        totals = report["summary"]["tokens"][name]
        lines.append(f"{name:<15} {'TOTAL':<12} {totals['nil_total']:>4} "
                     f"{totals['python_total']:>7} "
                     f"{totals['nil_total'] / totals['python_total']:>11.3f}")
    lines.extend(["", "Runtime ns/call (ratio < 1 favors NIL)",
                  "Case          NIL      Python   NIL/Python"])
    for case in report["cases"]:
        lines.append(f"{case['id']:<12} {case['nil']['median_ns_per_call']:>8.1f} "
                     f"{case['python']['median_ns_per_call']:>9.1f} "
                     f"{case['speed']['nil_over_python']:>11.3f}")
    summary = report["summary"]
    lines.append(f"NIL faster in {summary['nil_faster']}/{summary['cases']} cases.")
    lines.append("Speed measures NIL reference interpreter against CPython function calls; "
                 "small programs include call/measurement overhead.")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=HERE / "cases.json")
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
    report = benchmark(binary, args.iterations, args.warmup, args.repeats,
                       args.manifest)
    print(json.dumps(report, indent=2) if args.format == "json" else table(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
