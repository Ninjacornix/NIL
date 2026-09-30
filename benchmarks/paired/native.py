"""LLVM O0/O2 build-stage and in-process speed comparisons against Python/reference HIR."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import statistics
import subprocess
import tempfile
from pathlib import Path
import run as paired


def native_call(binary: Path, args: list[int], expected: int, iterations: int, repeats: int) -> list[float]:
    samples=[]
    for _ in range(repeats):
        output=subprocess.run([str(binary),'--bench',str(iterations),*map(str,args)],
                              check=True,capture_output=True,text=True,timeout=120)
        result=json.loads(output.stdout)
        if result['result']!=expected:
            raise ValueError('native benchmark result differs from reference')
        sample=result['ns_per_call']
        if not isinstance(sample,(int,float)) or sample<0:
            raise ValueError('invalid native timing sample')
        samples.append(sample)
    return samples


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--iterations',type=int,default=10000)
    parser.add_argument('--repeats',type=int,default=7)
    args=parser.parse_args()
    if not (1<=args.iterations<=10_000_000 and 1<=args.repeats<=100):
        parser.error('measurement bounds exceeded')
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil',
                    '--example','native_build'],cwd=paired.ROOT,check=True)
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil-compiler',
                    '--example','paired_runtime'],cwd=paired.ROOT,check=True)
    builder=paired.ROOT/'target/release/examples/native_build'
    reference=paired.ROOT/'target/release/examples/paired_runtime'
    counters,tokenizers=paired.load_tokenizers()
    manifest_path=paired.HERE/'cases-control-v2.json'
    manifest=json.loads(manifest_path.read_text())
    paired.validate_cases(manifest['cases'])
    cases=[]
    with tempfile.TemporaryDirectory(prefix='nil-native-bench-') as temporary:
        for case_index,case in enumerate(manifest['cases']):
            source=paired.HERE/case['nil'];py_file=paired.HERE/case['python']
            program=paired.load_program(py_file)
            native={};binaries={}
            for optimization in ['O0','O2']:
                binary=Path(temporary)/f"{case['id']}-{optimization}"
                output=subprocess.run([str(builder),case['profile'],str(source),str(case['function']),str(binary),optimization],
                                      check=True,capture_output=True,text=True,timeout=120)
                native[optimization]={'build':json.loads(output.stdout)};binaries[optimization]=binary
            for check in case['checks']:
                if program(*check['args'])!=check['expected']:
                    raise ValueError('Python differs from mathematical reference')
                paired.rust_call(reference,source,case['profile'],case['function'],check['expected'],0,1,1,check['args'])
                for binary in binaries.values():
                    out=subprocess.run([str(binary),*map(str,check['args'])],check=True,capture_output=True,text=True,timeout=120)
                    if int(out.stdout)!=check['expected']:
                        raise ValueError('native result differs from mathematical reference')
            timed=case['checks'][0]
            settings=(timed['expected'],1000,args.iterations,args.repeats,timed['args'])
            measurements={
                'interpreter':lambda:paired.rust_call(reference,source,case['profile'],case['function'],*settings),
                'python':lambda:paired.python_call(program,*settings),
                'O0':lambda:native_call(binaries['O0'],timed['args'],timed['expected'],args.iterations,args.repeats),
                'O2':lambda:native_call(binaries['O2'],timed['args'],timed['expected'],args.iterations,args.repeats),
            }
            order=list(measurements);offset=case_index%len(order);order=order[offset:]+order[:offset]
            samples={name:measurements[name]() for name in order}
            for optimization in native:
                native[optimization]['runtime_ns_per_call']=samples[optimization]
                native[optimization]['median_ns_per_call']=statistics.median(samples[optimization])
            python_ns=statistics.median(samples['python']);reference_ns=statistics.median(samples['interpreter'])
            cases.append({'id':case['id'],'profile':case['profile'],'checks':case['checks'],
                'timed_args':timed['args'],'measurement_order':order,
                'source':{'nil':paired.source_measure(source,counters),'python':paired.source_measure(py_file,counters)},
                'native':native,'python':{'runtime_ns_per_call':samples['python'],'median_ns_per_call':python_ns},
                'interpreter':{'runtime_ns_per_call':samples['interpreter'],'median_ns_per_call':reference_ns},
                'native_O2_over_python':native['O2']['median_ns_per_call']/python_ns,
                'native_O2_over_interpreter':native['O2']['median_ns_per_call']/reference_ns})
    result={'schema':1,'method':'per-call timings exclude process launch and compilation; identical bounded algorithms; native includes fuel/depth checks and fresh context per call',
            'settings':{'warmup':1000,'iterations':args.iterations,'repeats':args.repeats},
            'clang':subprocess.run([os.environ.get('NIL_CLANG','clang'),'--version'],check=True,capture_output=True,text=True).stdout,
            'tokenizers':tokenizers,'cases':cases,
            'manifest_sha256':hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
            'compiler_source_hashes':{str(p.relative_to(paired.ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(list((paired.ROOT/'crates').rglob('*.rs')) + list((paired.ROOT/'cli').rglob('*.rs')))},
            'environment':{'python':paired.platform.python_version(),'platform':paired.platform.platform(),
                           'git_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=paired.ROOT,text=True).strip(),
                           'git_dirty':bool(subprocess.check_output(['git','status','--porcelain'],cwd=paired.ROOT,text=True).strip())},
            'summary':{'cases':len(cases),'native_O2_faster_than_python':sum(c['native_O2_over_python']<1 for c in cases),
                       'native_O2_faster_than_interpreter':sum(c['native_O2_over_interpreter']<1 for c in cases)}}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(result['summary'])
    print('Raw report:',args.output)


if __name__=='__main__':
    main()
