import importlib.util
import json
from pathlib import Path
import sys
import unittest
import tempfile

HERE=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(HERE))
import typed

class TypedBenchTests(unittest.TestCase):
    def test_report_provenance_ignores_python_cache_directories(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/'__pycache__').mkdir()
            source=root/'sample.py'
            source.write_text('def program(a): return a\n')
            self.assertEqual(typed.program_files(root),[source])

    def test_flat_argument_bridge_keeps_mixed_parameter_order(self):
        self.assertEqual(typed.flatten([[1,2],True,7,[]]),[1,2,1,7])
        bridge=typed.bridge([2,'b','i'],2)
        self.assertIn('std::array<std::int64_t,2>{args[0],args[1]}',bridge)
        self.assertIn('args[2]!=0',bridge)
        self.assertIn('args[3]',bridge)
        self.assertIn('out[1]=result[1]',bridge)
        self.assertIn('std::abort()',bridge)

    def test_every_python_algorithm_matches_independent_checks_without_mutating_inputs(self):
        for name in ['cases.json', 'heldout.json']:
            manifest=json.loads((typed.CORPUS/name).read_text())
            for case in manifest['cases']:
                paths=[typed.CORPUS/'programs'/f"{case['id']}.py"]
                local=typed.CORPUS/'programs'/f"{case['id']}.local.py"
                if local.exists():
                    paths.append(local)
                for path in paths:
                    program=typed.paired.load_program(path)
                    for check in case['checks']:
                        with self.subTest(source=path.name,args=check['args']):
                            before=json.dumps(check['args'])
                            self.assertEqual(program(*check['args']),check['expected'])
                            self.assertEqual(json.dumps(check['args']),before)
