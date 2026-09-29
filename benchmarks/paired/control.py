"""M2 token, frontend and interpreter comparisons with equivalent Python algorithms."""
from __future__ import annotations
import argparse
import hashlib
import json
import statistics
import subprocess
from pathlib import Path
import run as paired


def assess(reports: list[dict]) -> dict:
    profiles = {r['cases'][0]['profile']:r for r in reports}
    baseline = {c['id']:c for c in profiles['expr-v0']['cases']}
    rows = []
    for c in profiles['expr-v2']['cases']:
        if c['id'] not in baseline:
            raise ValueError('profile corpus mismatch')
        for report in reports:
            peers = {p['id']:p for p in report['cases']}
            if set(peers) != set(baseline):
                raise ValueError('profile corpus mismatch')
            peer = peers[c['id']]
            if peer['checks'] != c['checks'] or peer['python']['source'] != c['python']['source']:
                raise ValueError('Python source/check vectors differ')
        tokens = {}
        for tokenizer,count in c['nil']['source']['tokens'].items():
            py = c['python']['source']['tokens'][tokenizer]
            v0 = baseline[c['id']]['nil']['source']['tokens'][tokenizer]
            if count >= py or count >= v0:
                raise ValueError(f"token reduction gate failed: {c['id']} {tokenizer}")
            tokens[tokenizer]={'expr_v2':count,'expr_v0':v0,'python':py}
        rows.append({'id':c['id'],'tokens':tokens})
    return {'passed':True,'gate':'every expr-v2 case strictly fewer tokens than equivalent Python and expr-v0 under both tokenizers','cases':rows}


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--iterations',type=int,default=1000)
    parser.add_argument('--repeats',type=int,default=7)
    parser.add_argument('--rounds',type=int,default=3)
    args=parser.parse_args()
    if not (1<=args.iterations<=100_000 and 1<=args.repeats<=100 and 1<=args.rounds<=20):
        parser.error('invalid measurement bounds')
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil-compiler',
                    '--example','paired_runtime','--example','paired_frontend'],cwd=paired.ROOT,check=True)
    binary=paired.ROOT/'target/release/examples/paired_runtime'
    frontend=paired.ROOT/'target/release/examples/paired_frontend'
    reports=[]
    for round_id in range(args.rounds):
        for v in [(round_id+i)%3 for i in range(3)]:
            report=paired.benchmark(binary,args.iterations,100,args.repeats,
                                    paired.HERE/f'cases-control-v{v}.json')
            report['round']=round_id
            for case in report['cases']:
                measured=subprocess.run([str(frontend),case['profile'],
                    str(paired.HERE/case['nil']['file']),str(args.iterations),str(args.repeats)],
                    check=True,capture_output=True,text=True,timeout=120)
                samples=json.loads(measured.stdout)['ns_per_compile']
                if len(samples)!=args.repeats or any(not isinstance(n,(int,float)) or n<0 for n in samples):
                    raise ValueError('invalid frontend timing samples')
                case['nil']['frontend_ns_per_compile']=samples
                case['nil']['frontend_median_ns']=statistics.median(samples)
            reports.append(report)
    counters,metadata=paired.load_tokenizers()
    screen=[]
    for case in reports[0]['cases']:
        name=case['id']
        source=(paired.HERE/f'control-samples/{name}.v2.nil').read_text()
        keyword=source.replace('@(','loop(')
        screen.append({'id':name,'symbol_source':source,'keyword_source':keyword,
                       'tokens':{t:{'symbol':c(source),'keyword':c(keyword)} for t,c in counters.items()}})
    # Retain the exact M1 corpus with the new interpreter; historic timings are not reused.
    m1=paired.benchmark(binary,10_000,1000,args.repeats,paired.HERE/'cases-experiment-v2.json')
    result={'schema':1,'reports':reports,'assessment':assess(reports),'m1_reference':m1,
            'loop_spelling_screen':screen,'tokenizers':metadata,
            'frontend_method':'100 warmups; complete parse, typed lowering, independent HIR validation; compiled result drop included',
            'compiler_source_hashes':{str(p.relative_to(paired.ROOT)):hashlib.sha256(p.read_bytes()).hexdigest()
                                      for p in sorted((paired.ROOT/'crates').rglob('*.rs'))}}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    for profile in ['expr-v0','expr-v1','expr-v2']:
        r=next(r for r in reports if r['cases'][0]['profile']==profile)
        print(profile,r['summary']['tokens'])
    print(result['assessment']['gate'])
    print('Raw report:',args.output)


if __name__=='__main__':
    main()
