#!/usr/bin/env python3
"""Interleaved A/B timing — the only accepted method for a speed claim.

Round 27 P9. Sequential measurement on this machine is not trustworthy: the
*same* binary measured 5.080x and 5.735x against C++ in two sessions with
byte-identical code, and a first version of the Round 26 collection gate was
reported as a 20% transform regression that interleaving showed to be noise.

So OLD and NEW alternate within one session, one pair at a time, and the
reported figure is the ratio of medians over at least fifteen pairs. Drift
affects both arms equally and cancels.
"""
import argparse
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys
import time

MINIMUM_PAIRS = 15


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def once(command):
    start = time.perf_counter()
    result = subprocess.run(command, capture_output=True)
    elapsed = (time.perf_counter() - start) * 1000.0
    return elapsed, result.returncode, result.stdout


def summarize(samples):
    return {
        "min": min(samples),
        "median": statistics.median(samples),
        "max": max(samples),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("old")
    parser.add_argument("new")
    parser.add_argument(
        "--task",
        action="append",
        default=[],
        help="argument passed to both binaries; repeat for several",
    )
    parser.add_argument("--pairs", type=int, default=MINIMUM_PAIRS)
    parser.add_argument("--output")
    parser.add_argument(
        "--allow-different-output",
        action="store_true",
        help="by default a speed claim requires both arms to agree",
    )
    arguments = parser.parse_args()
    if arguments.pairs < MINIMUM_PAIRS:
        parser.error(f"a speed claim needs at least {MINIMUM_PAIRS} pairs")

    old_command = [arguments.old, *arguments.task]
    new_command = [arguments.new, *arguments.task]
    old_samples, new_samples = [], []
    outputs = set()
    for _ in range(arguments.pairs):
        # Alternating within the pair keeps any drift symmetric.
        old_elapsed, old_code, old_out = once(old_command)
        new_elapsed, new_code, new_out = once(new_command)
        if old_code != 0 or new_code != 0:
            print(
                f"a binary failed: old exit {old_code}, new exit {new_code}",
                file=sys.stderr,
            )
            return 1
        outputs.add((old_out, new_out))
        old_samples.append(old_elapsed)
        new_samples.append(new_elapsed)

    differing = [pair for pair in outputs if pair[0] != pair[1]]
    if differing and not arguments.allow_different_output:
        print(
            "the two binaries disagree on output; a speed claim is meaningless",
            file=sys.stderr,
        )
        return 1

    old_stats, new_stats = summarize(old_samples), summarize(new_samples)
    ratio = new_stats["median"] / old_stats["median"]
    report = {
        "schema": 1,
        "pairs": arguments.pairs,
        "task": arguments.task,
        "old": {"path": arguments.old, "sha256": digest(arguments.old), **old_stats},
        "new": {"path": arguments.new, "sha256": digest(arguments.new), **new_stats},
        "ratio_of_medians": ratio,
        "outputs_agree": not differing,
    }
    print(
        f'{"arm":>5} {"min":>10} {"median":>10} {"max":>10}\n'
        f'{"old":>5} {old_stats["min"]:10.2f} {old_stats["median"]:10.2f} {old_stats["max"]:10.2f}\n'
        f'{"new":>5} {new_stats["min"]:10.2f} {new_stats["median"]:10.2f} {new_stats["max"]:10.2f}\n'
        f"ratio of medians new/old: {ratio:.3f}x over {arguments.pairs} interleaved pairs"
    )
    if arguments.output:
        pathlib.Path(arguments.output).write_text(json.dumps(report, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
