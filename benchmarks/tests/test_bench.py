import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('bench',Path(__file__).resolve().parents[1]/'bench.py')
bench=importlib.util.module_from_spec(spec);spec.loader.exec_module(bench)


class BenchmarkCommands(unittest.TestCase):
    def test_runs_use_unique_external_directories_and_respect_override(self):
        with tempfile.TemporaryDirectory() as temporary,patch.dict(os.environ,{'NIL_BENCH_HOME':temporary}):
            first=bench.output_directory('token');second=bench.output_directory('token')
            self.assertNotEqual(first,second)
            self.assertEqual(first.parent,Path(temporary).resolve()/'runs')
            with self.assertRaises(FileExistsError):bench.output_directory('token',first)

    def test_rejects_repository_output_before_creating_artifacts(self):
        with self.assertRaisesRegex(ValueError,'outside the repository'):
            bench.output_directory('runtime',bench.ROOT/'benchmarks/results/forbidden')
        self.assertFalse((bench.ROOT/'benchmarks/results/forbidden').exists())

    def test_provenance_identifies_compiler_and_ignores_caches(self):
        result=bench.provenance()
        self.assertIn('crates/nil-hir/src/lib.rs',result['source_hashes'])
        self.assertIn('benchmarks/paired/uv.lock',result['source_hashes'])
        self.assertFalse(any('/.venv/' in name or '/__pycache__/' in name for name in result['source_hashes']))

    def test_model_execution_requires_an_explicit_generation_mode(self):
        with self.assertRaises(SystemExit):bench.main(['generation','--smoke'])
        with self.assertRaises(SystemExit):bench.main(['runtime','--ollama','model'])
