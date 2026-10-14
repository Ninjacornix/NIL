#!/usr/bin/env python3
"""Regression tests for gates.py command-line and expected-count validation."""
import importlib.util
import pathlib
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("gates", ROOT / "scripts/gates.py")
gates = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gates)


class GateValidationTests(unittest.TestCase):
    def test_non_positive_jobs_are_rejected_before_creating_output(self):
        for jobs in ("0", "-1"):
            with tempfile.TemporaryDirectory() as temporary:
                output = pathlib.Path(temporary) / "out"
                with self.assertRaises(SystemExit) as error:
                    gates.main(["fast", "--out", str(output), "--jobs", jobs])
                self.assertEqual(error.exception.code, 2)
                self.assertFalse(output.exists())

    def test_missing_expected_gates_and_counts_are_regressions(self):
        expected = {
            "fuzz": {"programs": 100, "divergences": 0, "families": 22},
            "native": {"tests": 20},
        }
        actual = {
            "fuzz": {"counts": {"programs": 110, "divergences": 0}},
        }
        self.assertEqual(
            gates.count_regressions(expected, actual),
            ["fuzz.families: None < 22", "native.tests: None < 20"],
        )


if __name__ == "__main__":
    unittest.main()
