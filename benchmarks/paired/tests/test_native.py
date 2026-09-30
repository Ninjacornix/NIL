import json
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import native

class NativeBenchmarkTests(unittest.TestCase):
    def test_rejects_incorrect_native_result(self):
        output=SimpleNamespace(stdout=json.dumps({'result':99,'ns_per_call':1.0}))
        with patch.object(native.subprocess,'run',return_value=output):
            with self.assertRaisesRegex(ValueError,'differs from reference'):
                native.native_call(Path('/unused'),[1],42,10,1)
    def test_rejects_invalid_timing(self):
        output=SimpleNamespace(stdout=json.dumps({'result':42,'ns_per_call':-1}))
        with patch.object(native.subprocess,'run',return_value=output):
            with self.assertRaisesRegex(ValueError,'invalid native timing'):
                native.native_call(Path('/unused'),[1],42,10,1)
