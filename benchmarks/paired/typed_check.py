"""Measure frontend/checker latency separately from typed entry payload size."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import typed
import run as paired


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil-compiler','--example','typecheck_bench'],cwd=paired.ROOT,check=True)
    helper=paired.ROOT/'target/release/examples/typecheck_bench'
    reports=[]
    for case in json.loads((typed.CORPUS/'cases.json').read_text())['cases']:
        source=typed.CORPUS/'programs'/f"{case['id']}.nil"
        stats=json.loads(subprocess.check_output([str(helper),str(source)],text=True,timeout=120))
        reports.append({'id':case['id'],'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),**stats})
    files=[Path(__file__),paired.ROOT/'crates/nil-compiler/examples/typecheck_bench.rs',*list((paired.ROOT/'crates/nil-compiler/src').glob('*.rs')),*list((paired.ROOT/'crates/nil-hir/src').glob('*.rs'))]
    report={'schema':1,'profile':'expr-v4','method':'101 repeated in-process frontend compilations and independent HIR validations per program; report medians. Source file I/O, process launch, input HIR cloning and result destruction are excluded. Frontend includes parsing/resolution/type checking/validation. Entry payload bytes use flat i64 slots, not host object memory size. Small timings are exploratory and not regression thresholds.',
            'environment':{'platform':paired.platform.platform(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip()},
            'cases':reports,'source_hashes':{str(p.relative_to(paired.ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files}}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
    print('Raw validation/payload results:',args.output)

if __name__=='__main__':main()
