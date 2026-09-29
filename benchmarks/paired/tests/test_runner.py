import importlib.util
import tempfile
import unittest
from pathlib import Path

import tiktoken


RUNNER = Path(__file__).resolve().parents[1] / "run.py"
spec = importlib.util.spec_from_file_location("paired_runner", RUNNER)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class PairedRunnerTests(unittest.TestCase):
    def test_source_measure_counts_exact_tokens_and_unicode(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.nil"
            path.write_text("hello\nπ", encoding="utf-8", newline="\n")
            source = runner.source_measure(path, tiktoken.get_encoding("cl100k_base"))
        self.assertEqual(source["bytes"], 8)
        self.assertEqual(source["characters"], 7)
        self.assertEqual(source["tokens"], 3)

    def test_comparison_marks_lower_value_as_better(self):
        self.assertEqual(runner.comparison(5, 10),
                         {"nil_over_python": 0.5, "nil_better": True})
        self.assertFalse(runner.comparison(10, 10)["nil_better"])

    def test_manifest_requires_nonempty_checks_and_unique_ids(self):
        case = {"id": "a", "checks": [{"args": [1], "expected": 1}]}
        runner.validate_cases([case])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            runner.validate_cases([case, case])
        with self.assertRaisesRegex(ValueError, "no checks"):
            runner.validate_cases([{"id": "a", "checks": []}])


if __name__ == "__main__":
    unittest.main()
