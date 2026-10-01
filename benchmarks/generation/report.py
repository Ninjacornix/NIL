"""Audit and describe completed local generation trials without excluding failures."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import experiment as e


def load_lines(path):
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def audit(root):
    config=json.loads((root/'config.json').read_text())
    trials=load_lines(root/'trials.jsonl');attempts=load_lines(root/'attempts.jsonl')
    expected={(m,p,t,s) for m in config['models'] for p in config['profiles'] for t in config['tasks'] for s in config['seeds']}
    actual=[(r['model'],r['profile'],r['task'],r['seed']) for r in trials]
    assert len(actual)==len(set(actual)), 'duplicate trials'
    assert set(actual)==expected, 'incomplete experiment'
    for path,sha in config['source_hashes'].items():
        assert hashlib.sha256((e.ROOT/path).read_bytes()).hexdigest()==sha, 'changed measured source: '+path
    grouped={}
    for r in attempts: grouped.setdefault((r['model'],r['profile'],r['task'],r['seed']),[]).append(r)
    for trial in trials:
        key=trial['model'],trial['profile'],trial['task'],trial['seed']
        rows=grouped[key]
        assert [r['attempt'] for r in rows]==list(range(len(rows)))
        assert len(rows)==trial['attempts']<=config['attempt_limit']
        assert trial['solved']==rows[-1]['semantic_ok']
        assert not any(r['semantic_ok'] for r in rows[:-1]), 'continued solved trial'
        if all(r['generated_tokens'] is not None for r in rows):
            assert sum(r['generated_tokens'] for r in rows)==trial['output_tokens']
            assert sum(r['generated_tokens'] for r in rows[1:])==trial['repair_tokens']
            assert trial['output_tokens']<=config['output_budget']
        if all(r['input_tokens'] is not None for r in rows):
            assert sum(r['input_tokens'] for r in rows)==trial['input_tokens']
        for r in rows:
            if r.get('semantic_ok'):
                assert r['tests_total']>0 and r['tests_passed']==r['tests_total']==r['native_tests_passed']
                assert r['parse_ok'] and r['typecheck_ok'] and r['compile_ok']
            if r.get('source_sha256'):
                trial_id=r['trial_id'];attempt=r['attempt']
                source=(root/trial_id/f'source-{attempt}.nil').read_bytes().decode('utf-8')
                response=json.loads((root/trial_id/f'response-{attempt}.json').read_text())
                request=json.loads((root/trial_id/f'request-{attempt}.json').read_text())
                assert e.digest(source)==r['source_sha256']
                assert e.digest(json.dumps(request['messages'],sort_keys=True))==r['prompt_sha256']
                assert response.get('eval_count')==r['generated_tokens']
                assert response.get('prompt_eval_count')==r['input_tokens']
    return config,trials,attempts


def main(root):
    config,trials,attempts=audit(root)
    rows=e.summary(trials)
    intervals=[e.paired_interval(trials,m,'expr-v0',p) for m in config['models'] for p in config['profiles'][1:]]
    lines=['# Local Ollama representation pilot', '',
        f"Completed {len(trials)} trials and {len(attempts)} attempts. Models, tasks, budgets and prompts are frozen in [configuration](config.json). Raw failures are retained in [attempts](attempts.jsonl) and [trials](trials.jsonl).",'',
        '| Model | Profile | Solved | First pass | Output TCR | Input + output TCR | Repair tokens |',
        '|---|---|---:|---:|---:|---:|---:|']
    number=lambda n:'undefined' if n is None else f'{n:.1f}'
    for r in rows:
        lines.append(f"| {r['model']} | {r['profile']} | {r['solved']}/{r['trials']} | {r['first_pass']:.1%} | {number(r['tcr'])} | {number(r['tcr_total'])} | {r['repair_tokens']} |")
    lines+=['','Failed trials contribute their full token cost. Zero solves make TCR undefined; unknown provider usage invalidates exact accounting. Provider totals include all returned output, not just extracted code. Source tokens remain a separate pinned-tokenizer screen.','',
        '| Model | Candidate vs v0 | TCR ratio 95% interval | Solve-rate difference 95% interval | Undefined resamples | Gate |',
        '|---|---|---|---|---:|---|']
    gates=[]
    for r in intervals:
        tcr=r['tcr_ratio_95'];solve=r['solve_rate_difference_95']
        gate=bool(tcr and solve and not r['undefined_tcr_resamples'] and tcr[1]<1-config['tcr_improvement'] and solve[0]>-config['correctness_margin'])
        gates.append(gate)
        lines.append(f"| {r['model']} | {r['candidate']} | {tcr} | {solve} | {r['undefined_tcr_resamples']} | {'passes' if gate else 'not established'} |")
    lines+=['','## First-attempt stage success','',
        '| Model | Profile | Parse | Type check | Native compile | Semantically correct |',
        '|---|---|---:|---:|---:|---:|']
    for row in rows:
        first=[r for r in attempts if r['model']==row['model'] and r['profile']==row['profile'] and r['attempt']==0]
        stage=[sum(r.get(key) is True for r in first) for key in ['parse_ok','typecheck_ok','compile_ok','semantic_ok']]
        lines.append(f"| {row['model']} | {row['profile']} | {stage[0]}/{len(first)} | {stage[1]}/{len(first)} | {stage[2]}/{len(first)} | {stage[3]}/{len(first)} |")
    curves=[]
    for row in rows:
        selected=[r for r in trials if r['model']==row['model'] and r['profile']==row['profile']]
        curves.append(dict(model=row['model'],profile=row['profile'],solves_at_observed_output_budget={
            str(budget):sum(r['solved'] and r['output_tokens'] is not None and r['output_tokens']<=budget for r in selected)
            for budget in [64,128,256,512,640]},trials=len(selected)))
    (root/'observed-budget-curves.json').write_text(json.dumps(curves,indent=2)+'\n')
    lines+=['','Observed cumulative-output solve curves are recorded separately; these are not counterfactual reruns under smaller request limits. Stage rates use all initial trials, with skipped stages counting as no stage success.']
    failures=Counter(r.get('failure_reason') for r in attempts if not r['semantic_ok'])
    lines+=['','## Failure evidence','',str(dict(sorted(failures.items()))),'',
        'The intervals resample whole tasks, preserving seeds/profile pairing. Undefined TCR resamples cannot support selection. The pilot is small and uses familiar numerical tasks; no claim of contamination-proof generalization follows. No grammar or default has been changed.','',
        '## Verification','',
        'The audit checks complete paired cells, attempt ordering, no repairs after success, output/repair/input arithmetic, request/response/source hashes, immutable compiler provenance, budget limits and nonempty reference/native correctness vectors. Model digests and Ollama details are retained in configuration. Native process wall times are not steady-state runtime measurements.','',
        'See [method and reproduction](../../../README.md).']
    (root/'analysis-provenance.json').write_text(json.dumps({'report_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'experiment_sha256':hashlib.sha256(Path(e.__file__).read_bytes()).hexdigest(),'config_sha256':hashlib.sha256((root/'config.json').read_bytes()).hexdigest()},indent=2)+'\n')
    (root/'REPORT.md').write_text('\n'.join(lines)+'\n')
    (root/'summary.json').write_text(json.dumps(rows,indent=2)+'\n')
    (root/'paired-intervals.json').write_text(json.dumps(intervals,indent=2)+'\n')
    print('Audit passed:',len(trials),'trials;',len(attempts),'attempts;',sum(gates),'model/candidate gates passed')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('run',type=Path)
    main(parser.parse_args().run)
