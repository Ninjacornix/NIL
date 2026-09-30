"""Repeat the compact-profile experiment, including competitive Python lambdas."""
from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import subprocess
from pathlib import Path

import run as paired


def compare(reports: list[dict], lambda_sources: dict[str, dict]) -> dict:
    by_profile = {report['cases'][0]['profile']: report for report in reports}
    baseline = by_profile['expr-v0']
    candidate = by_profile['expr-v2']
    baseline_cases = {c['id']: c for c in baseline['cases']}
    if any(set(baseline_cases) != {c['id'] for c in report['cases']} for report in reports):
        raise ValueError('profiles must have identical case IDs')
    rows = []
    for c in candidate['cases']:
        original = baseline_cases[c['id']]
        for report in reports:
            peer = next(p for p in report['cases'] if p['id'] == c['id'])
            if c['checks'] != peer['checks'] or c['python']['source'] != peer['python']['source']:
                raise ValueError('profiles must use identical Python sources and checks')
        row = {'id': c['id'], 'tokens': {}}
        for tokenizer in c['nil']['source']['tokens']:
            count = c['nil']['source']['tokens'][tokenizer]
            v0 = original['nil']['source']['tokens'][tokenizer]
            python = c['python']['source']['tokens'][tokenizer]
            python_lambda = lambda_sources[c['id']]['source']['tokens'][tokenizer]
            if not count < min(v0, python, python_lambda):
                raise ValueError(f"candidate fails token gate: {c['id']} {tokenizer}")
            row['tokens'][tokenizer] = {
                'expr_v0': v0, 'expr_v2': count, 'python_def': python,
                'python_lambda': python_lambda,
                'reduction_vs_v0': 1 - count/v0,
                'reduction_vs_python_def': 1 - count/python,
                'reduction_vs_python_lambda': 1 - count/python_lambda,
            }
        rows.append(row)
    return {'token_gate': 'expr-v2 strictly fewer tokens per case than expr-v0 and both Python forms, for both tokenizers',
            'passed': True, 'cases': rows}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--iterations', type=int, default=50_000)
    parser.add_argument('--warmup', type=int, default=5_000)
    parser.add_argument('--repeats', type=int, default=9)
    args = parser.parse_args()
    if not (1 <= args.rounds <= 20 and 1 <= args.iterations <= 10_000_000
            and 0 <= args.warmup <= 10_000_000 and 1 <= args.repeats <= 100):
        parser.error('invalid measurement bounds')
    subprocess.run(['cargo', 'build', '--release', '--locked', '--offline', '-p',
                    'nil-compiler', '--example', 'paired_runtime'], cwd=paired.ROOT, check=True)
    binary = paired.ROOT / 'target/release/examples/paired_runtime'
    reports = []
    # Rotate profile order to reduce systematic temperature/order bias.
    for round_id in range(args.rounds):
        for v in [(round_id+i) % 3 for i in range(3)]:
            report = paired.benchmark(binary, args.iterations, args.warmup, args.repeats,
                                      paired.HERE / f'cases-experiment-v{v}.json')
            report['round'] = round_id
            reports.append(report)
    counters, _ = paired.load_tokenizers()
    lambdas = {}
    manifest = json.loads((paired.HERE / 'cases-experiment-v2.json').read_text())
    for case in manifest['cases']:
        path = paired.HERE / case['python'].replace('.compact.py', '.lambda.py')
        program = paired.load_program(path)
        for check in case['checks']:
            if program(*check['args']) != check['expected']:
                raise ValueError(f"lambda correctness failed: {case['id']}")
        timed = case['checks'][0]
        samples = paired.python_call(program, timed['expected'], args.warmup,
                                     args.iterations, args.repeats, timed['args'])
        lambdas[case['id']] = {'file': str(path.relative_to(paired.HERE)),
                              'source': paired.source_measure(path,counters),
                              'runtime_ns_per_call': samples,
                              'median_ns_per_call': statistics.median(samples)}
    result = {'schema': 1, 'reports': reports, 'python_lambda': lambdas,
              'comparison': compare(reports[:3],lambdas)}
    result['compiler_source_hashes'] = {
        str(p.relative_to(paired.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted((paired.ROOT/'crates').rglob('*.rs'))}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    for profile in ['expr-v0','expr-v1','expr-v2']:
        report=next(r for r in reports if r['cases'][0]['profile']==profile)
        print(profile,report['summary']['tokens'])
    print('Token gate passed on every case for both tokenizers.')
    print(f'Raw reports: {args.output}')


if __name__ == '__main__':
    main()
