"""Compare value semantics with equivalent fresh-local mutation baselines."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import tempfile
import os
import typed
import native
import run as paired


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--iterations',type=int,default=200000)
    parser.add_argument('--python-iterations',type=int,default=2000)
    parser.add_argument('--repeats',type=int,default=7)
    args=parser.parse_args()
    if not (1<=args.iterations<=10000000 and 1<=args.python_iterations<=10000000 and 1<=args.repeats<=100):
        parser.error('measurement bounds exceeded')
    generator=typed.CORPUS/'generate.py'
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil','--example','native_build'],cwd=paired.ROOT,check=True)
    builder=paired.ROOT/'target/release/examples/native_build'
    cases=[c for c in json.loads((typed.CORPUS/'cases.json').read_text())['cases'] if c['id'].startswith(('reverse','prefix'))]
    counters,tokenizers=paired.load_tokenizers();reports=[];provenance={}
    with tempfile.TemporaryDirectory(prefix='nil-typed-local-') as temp:
        folder=Path(temp)
        for index,case in enumerate(cases):
            name=case['id']
            paths={ext:typed.CORPUS/'programs'/f'{name}.local.{ext}' for ext in ['py','cpp']}
            for p in paths.values(): provenance[str(p.relative_to(paired.ROOT))]=hashlib.sha256(p.read_bytes()).hexdigest()
            binary=folder/(name+'-nil');rt=folder/(name+'.c')
            result=subprocess.run([str(builder),'expr-v4',str(typed.CORPUS/'programs'/f'{name}.nil'),'0',str(binary),'O2','unbounded',str(rt)],check=True,capture_output=True,text=True,timeout=120)
            cpp_binary,cpp_stats=typed.cpp_build(folder,case|{'id':name+'.local'},rt)
            program=paired.load_program(paths['py'])
            for check in case['checks']:
                before=json.dumps(check['args'])
                if program(*check['args'])!=check['expected'] or json.dumps(check['args'])!=before: raise ValueError('local Python changed value semantics')
                for b in [binary,cpp_binary]:
                    output=subprocess.check_output([str(b),*map(str,typed.flatten(check['args']))],text=True,timeout=120)
                    if json.loads(output)!=check['expected']: raise ValueError('local native differs from oracle')
            samples={k:[] for k in ['nil','cpp_local','python_local']};orders=[];check=case['checks'][0]
            for repeat in range(args.repeats):
                order=list(samples);offset=(index+repeat)%len(order);order=order[offset:]+order[:offset];orders.append(order)
                for label in order:
                    if label=='python_local': value=typed.measure_python(program,check['args'],check['expected'],args.python_iterations)
                    else: value=native.native_call(binary if label=='nil' else cpp_binary,typed.flatten(check['args']),check['expected'],args.iterations,1)[0]
                    samples[label].append(value)
            measurements={k:{'runtime_ns_per_call':v,'median_ns_per_call':statistics.median(v)} for k,v in samples.items()}
            measurements['nil']['build']=json.loads(result.stdout);measurements['cpp_local']['build']=cpp_stats
            reports.append({**case,'measurement_orders':orders,'measurements':measurements,
                'sources':{'nil':paired.source_measure(typed.CORPUS/'programs'/f'{name}.nil',counters),**{k+'_local':paired.source_measure(p,counters) for k,p in paths.items()}},
                'nil_over_cpp_local':measurements['nil']['median_ns_per_call']/measurements['cpp_local']['median_ns_per_call'],
                'nil_over_python_local':measurements['nil']['median_ns_per_call']/measurements['python_local']['median_ns_per_call']})
            print('Measured local',name,flush=True)
    for p in [Path(__file__),paired.HERE/'typed.py',generator,*list((paired.ROOT/'crates').rglob('*.rs')),*list((paired.ROOT/'cli').rglob('*.rs'))]:
        provenance[str(p.relative_to(paired.ROOT))]=hashlib.sha256(p.read_bytes()).hexdigest()
    report={'schema':1,'settings':vars(args)|{'output':str(args.output),'warmup':1000},'tokenizers':tokenizers,
        'method':'Same unbounded NIL O2 and C++ O2 -fwrapv with actual generated C driver/flat bridge, separate translation units without LTO. Fresh unaliased local result arrays are mutated in place in C++/Python, preserving externally observable immutable value semantics. Same checked reads and independent fixtures as typed.py; output comparison and marshaling timed, process/compilation excluded. Python numeric semantics equivalent only on the fixture domains. This is a separate interleaved timing run, not a direct comparison of samples between runs.',
        'source_hashes':provenance,'cases':reports,'environment':{'platform':paired.platform.platform(),'python':paired.platform.python_version(),'clang':subprocess.check_output([os.environ.get('NIL_CLANG','clang'),'--version'],text=True)},
        'unmeasured':['model generation/repair cost','TCR']}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
    print('Raw results:',args.output)

if __name__=='__main__':main()
