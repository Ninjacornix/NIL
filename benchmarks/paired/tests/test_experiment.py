import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import experiment


class ExperimentTests(unittest.TestCase):
    def inputs(self):
        reports = []
        for profile, count in [('expr-v0', 12), ('expr-v1', 10), ('expr-v2', 9)]:
            reports.append({'cases': [{'id': 'a', 'profile': profile,
                'checks': [{'args': [1], 'expected': 1}],
                'python': {'source': {'tokens': {'cl100k_base': 14, 'qwen2.5_coder': 14}}},
                'nil': {'source': {'tokens': {'cl100k_base': count, 'qwen2.5_coder': count}}}}]})
        lambdas = {'a': {'source': {'tokens': {'cl100k_base': 11, 'qwen2.5_coder': 11}}}}
        return reports, lambdas

    def test_token_gate_requires_strict_improvement_on_both_tokenizers(self):
        reports, lambdas = self.inputs()
        self.assertTrue(experiment.compare(reports, lambdas)['passed'])
        reports[2]['cases'][0]['nil']['source']['tokens']['qwen2.5_coder'] = 11
        with self.assertRaisesRegex(ValueError, 'token gate'):
            experiment.compare(reports, lambdas)

    def test_changed_check_vectors_are_rejected_for_any_profile(self):
        reports, lambdas = self.inputs()
        reports[1]['cases'][0]['checks'][0]['expected'] = 2
        with self.assertRaisesRegex(ValueError, 'identical Python sources and checks'):
            experiment.compare(reports, lambdas)

    def test_mismatched_corpus_is_rejected(self):
        reports, lambdas = self.inputs()
        reports[1]['cases'][0]['id'] = 'b'
        with self.assertRaisesRegex(ValueError, 'identical case IDs'):
            experiment.compare(reports, lambdas)

    def test_changed_python_baseline_is_rejected(self):
        reports, lambdas = self.inputs()
        reports[0]['cases'][0]['python'] = copy.deepcopy(reports[0]['cases'][0]['python'])
        reports[0]['cases'][0]['python']['source']['tokens']['cl100k_base'] = 20
        with self.assertRaisesRegex(ValueError, 'identical Python sources and checks'):
            experiment.compare(reports, lambdas)
