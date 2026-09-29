import copy
import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import control

class ControlBenchmarkTests(unittest.TestCase):
    def reports(self):
        case={'id':'max','checks':[{'args':[1,2],'expected':2}],
              'python':{'source':{'tokens':{'cl100k_base':13,'qwen2.5_coder':13}}},
              'nil':{'source':{'tokens':{'cl100k_base':6,'qwen2.5_coder':6}}}}
        reports=[]
        for profile in ['expr-v0','expr-v1','expr-v2']:
            c=copy.deepcopy(case);c['profile']=profile
            if profile=='expr-v0':c['nil']['source']['tokens']={t:11 for t in c['nil']['source']['tokens']}
            reports.append({'cases':[c]})
        return reports
    def test_requires_reduction_under_both_tokenizers(self):
        reports=self.reports()
        self.assertTrue(control.assess(reports)['passed'])
        reports[2]['cases'][0]['nil']['source']['tokens']['qwen2.5_coder']=13
        with self.assertRaisesRegex(ValueError,'token reduction gate'):control.assess(reports)
    def test_rejects_changed_reference_outputs(self):
        reports=self.reports();reports[1]['cases'][0]['checks'][0]['expected']=99
        with self.assertRaisesRegex(ValueError,'vectors differ'):control.assess(reports)
    def test_rejects_mismatched_corpus(self):
        reports=self.reports();reports[1]['cases'][0]['id']='other'
        with self.assertRaisesRegex(ValueError,'corpus mismatch'):control.assess(reports)
