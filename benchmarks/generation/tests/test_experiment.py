import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import contextlib
import io
import json

HERE=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('experiment',HERE/'experiment.py')
e=importlib.util.module_from_spec(spec);spec.loader.exec_module(e)


def trial(profile='expr-v0',task='a',seed=17,solved=False,output=100,inputs=200,repair=0,attempts=1):
    return dict(model='model',profile=profile,task=task,seed=seed,solved=solved,
                output_tokens=output,input_tokens=inputs,repair_tokens=repair,attempts=attempts)


class Metrics(unittest.TestCase):
    def test_failed_trials_are_charged_and_reasoning_not_added(self):
        report=e.summary([trial(solved=True,output=20),trial(output=100)])[0]
        self.assertEqual(report['tcr'],120)
        self.assertEqual(report['tcr_total'],520)
        self.assertEqual(report['solve_rate'],.5)
        self.assertEqual(report['median_solved_ttCP'],20)

    def test_zero_solves_and_missing_usage_are_not_zero_cost(self):
        self.assertIsNone(e.summary([trial()])[0]['tcr'])
        self.assertIsNone(e.summary([trial(solved=True),trial(output=None)])[0]['tcr'])

    def test_repairs_contribute_to_ttCP_and_reduce_first_pass(self):
        report=e.summary([trial(solved=True,output=90,repair=70,attempts=2)])[0]
        self.assertEqual(report['tcr'],90)
        self.assertEqual(report['repair_tokens'],70)
        self.assertEqual(report['first_pass'],0)

    def test_paired_bootstrap_retains_failure_cost_and_tasks(self):
        rows=[trial(profile=p,task=t,seed=s,solved=True,output=n) for p,n in [('expr-v0',100),('expr-v2',50)] for t in ['a','b'] for s in [17,29]]
        result=e.paired_interval(rows,'model','expr-v0','expr-v2',100)
        self.assertEqual(result['tcr_ratio_95'],[.5,.5])
        self.assertEqual(result['solve_rate_difference_95'],[0,0])
        with self.assertRaises(ValueError): e.paired_interval(rows[:-1],'model','expr-v0','expr-v2',100)

    def test_extraction_does_not_discard_extra_output(self):
        self.assertEqual(e.extract_source('```nil\n1=a+1\n```'),'1=a+1\n')
        self.assertIn('Explanation',e.extract_source('Explanation\n```nil\n1=a+1\n```'))


class Oracle(unittest.TestCase):
    def test_real_pipeline_valid_parse_type_semantic_and_fuel_failures(self):
        task=dict(checks=[dict(args=[5],expected=6),dict(args=[0],expected=1)])
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)
            correct=e.evaluate('1=a+1\n','expr-v2',task,path)
            self.assertTrue(correct['semantic_ok'],correct)
            self.assertTrue(correct['compile_ok'])
            self.assertEqual(correct['native_tests_passed'],2)
            parse=e.evaluate('garbage','expr-v2',task,path)
            self.assertFalse(parse['parse_ok'])
            self.assertIsNone(parse['typecheck_ok'])
            check=e.evaluate('1=true+1','expr-v2',task,path)
            self.assertTrue(check['parse_ok']);self.assertFalse(check['typecheck_ok'])
            wrong=e.evaluate('1=a+2','expr-v2',task,path)
            self.assertTrue(wrong['compile_ok']);self.assertFalse(wrong['semantic_ok'])
            exhausted=e.evaluate('1=@(a;true;a;a)','expr-v2',task,path)
            self.assertFalse(exhausted['semantic_ok'])
            self.assertNotIn('expected',exhausted['feedback'])

    def test_fixture_programs_work_in_all_profiles(self):
        import json
        tasks=json.loads((HERE/'tasks.json').read_text())['tasks'][:8]
        with tempfile.TemporaryDirectory() as directory:
            for task in tasks:
                for p in e.PROFILES:
                    version=p[-1]
                    source=(e.ROOT/'benchmarks/paired/control-samples'/f"{task['id']}.v{version}.nil").read_text()
                    with self.subTest(task=task['id'],profile=p):
                        result=e.evaluate(source,p,task,Path(directory))
                        self.assertTrue(result['semantic_ok'],result)


class Trajectories(unittest.TestCase):
    def test_runner_enforces_budget_logs_all_repairs_and_preserves_zero_solves(self):
        def provider(endpoint,path,body=None):
            if path=='/api/tags': return {'models':[{'name':'mock','digest':'test'}]}
            if path=='/api/version': return {'version':'test'}
            if path=='/api/show': return {}
            return {'message':{'content':'invalid source'},'eval_count':body['options']['num_predict'],
                    'prompt_eval_count':100,'done_reason':'length'}
        with tempfile.TemporaryDirectory() as directory:
            import sys
            sys.path.insert(0,str(e.ROOT/'benchmarks/paired'))
            import run
            args=SimpleNamespace(endpoint='unused',models=['mock'],seeds=[17],task_limit=1,output=Path(directory))
            with patch.object(e,'post',provider), patch.object(e,'evaluate',return_value=dict(semantic_ok=False,failure_reason='parse',feedback='Invalid syntax')), patch('run.load_tokenizers',return_value=({'test':len},[{'id':'test'}])), contextlib.redirect_stdout(io.StringIO()):
                e.run(args)
            records=[json.loads(line) for line in (args.output/'attempts.jsonl').read_text().splitlines()]
            self.assertEqual(len(records),9)
            for profile in e.PROFILES:
                rows=[r for r in records if r['profile']==profile]
                self.assertEqual([r['generated_tokens'] for r in rows],[256,256,128])
                self.assertEqual([r['repair_tokens'] for r in rows],[0,256,128])
                self.assertEqual(rows[-1]['cumulative_output_tokens'],640)
            for report in json.loads((args.output/'summary.json').read_text()):
                self.assertEqual(report['solved'],0)
                self.assertEqual(report['output_tokens'],640)
                self.assertIsNone(report['tcr'])


class Audit(unittest.TestCase):
    def test_completed_audit_rejects_missing_cells_and_incorrect_usage(self):
        import sys
        sys.path.insert(0,str(HERE))
        import report
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            config=dict(models={'mock':{}},profiles=['expr-v0'],tasks=['task'],seeds=[17],
                        source_hashes={},attempt_limit=3,output_budget=640)
            (root/'config.json').write_text(json.dumps(config))
            row=trial(task='task',output=20,inputs=40)
            record=dict(model='model',profile='expr-v0',task='task',seed=17,attempt=0,
                        semantic_ok=False,generated_tokens=20,input_tokens=40)
            # Model names must agree before the audit can accept a completed matrix.
            row['model']='mock';record['model']='mock'
            (root/'trials.jsonl').write_text(json.dumps(row)+'\n')
            (root/'attempts.jsonl').write_text(json.dumps(record)+'\n')
            self.assertEqual(len(report.audit(root)[1]),1)
            row['output_tokens']=19
            (root/'trials.jsonl').write_text(json.dumps(row)+'\n')
            with self.assertRaises(AssertionError): report.audit(root)
            (root/'trials.jsonl').write_text('')
            with self.assertRaises(AssertionError): report.audit(root)
