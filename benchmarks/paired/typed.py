"""Typed v4 source-token, compilation, and runtime comparisons with Python/C++."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import tempfile
import time
import native
import run as paired
from cpp import invoke

CORPUS = paired.HERE/'typed'

def program_files(root):
    return [p for p in root.glob('*') if p.is_file()]


def flatten(args):
    return [v for arg in args for v in (arg if isinstance(arg,list) else [int(arg)])]


def bridge(parameters, result):
    expressions=[];slot=0
    for ty in parameters:
        if isinstance(ty,int):
            expressions.append(f'std::array<std::int64_t,{ty}>{{'+','.join(f'args[{slot+i}]' for i in range(ty))+'}')
            slot+=ty
        else:
            expressions.append(f'args[{slot}]' if ty=='i' else f'args[{slot}]!=0');slot+=1
    stores=''.join(f'out[{i}]=result[{i}];' for i in range(result)) if isinstance(result,int) else 'out[0]=result;'
    guards='';slot=0
    for ty in parameters:
        if ty=='b': guards+=f'if(args[{slot}]!=0 && args[{slot}]!=1) std::abort();'
        slot+=ty if isinstance(ty,int) else 1
    return ('\nextern "C" void nil_entry(void*,const std::int64_t* args,std::int64_t* out) {'
            +guards+'auto result=program('+','.join(expressions)+');'+stores+'}\n')


def cpp_build(folder, case, runtime):
    clang=os.environ.get('NIL_CLANG','clang');name=case['id']
    source=folder/(name+'.cpp');obj=folder/(name+'.o');rt=folder/(name+'-runtime.o');binary=folder/(name+'-cpp')
    source.write_text((CORPUS/'programs'/f'{name}.cpp').read_text()+bridge(case['parameters'],case['result']))
    compile_ms=invoke([clang,'-std=c++17','-O2','-fwrapv','-x','c++','-c',str(source),'-o',str(obj)])
    runtime_ms=invoke([clang,'-std=c11','-O2','-c',str(runtime),'-o',str(rt)])
    link_ms=invoke([clang,str(obj),str(rt),'-o',str(binary)])
    return binary,{'cpp_compile_ms':compile_ms,'runtime_compile_ms':runtime_ms,'link_ms':link_ms,
                   'backend_total_ms':compile_ms+runtime_ms+link_ms,'binary_bytes':binary.stat().st_size,
                   'runtime_sha256':hashlib.sha256(runtime.read_bytes()).hexdigest(),
                   'bridge_sha256':hashlib.sha256(bridge(case['parameters'],case['result']).encode()).hexdigest()}


def measure_python(program, args, expected, iterations):
    for _ in range(1000):
        if program(*args)!=expected: raise ValueError('Python warmup mismatch')
    start=time.perf_counter_ns()
    for _ in range(iterations):
        if program(*args)!=expected: raise ValueError('Python timed mismatch')
    return (time.perf_counter_ns()-start)/iterations


def source_metrics(case, counters):
    return {lang:paired.source_measure(CORPUS/'programs'/f"{case['id']}.{ext}",counters)
            for lang,ext in [('nil','nil'),('python','py'),('cpp','cpp')]}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--iterations',type=int,default=200000)
    parser.add_argument('--o0-iterations',type=int,default=2000)
    parser.add_argument('--python-iterations',type=int,default=2000)
    parser.add_argument('--repeats',type=int,default=7)
    parser.add_argument('--baseline',choices=['value','local'],default='value',
                        help='local uses faster fresh-local transform baselines')
    parser.add_argument('--manifest',type=Path,default=CORPUS/'cases.json')
    args=parser.parse_args()
    if not (1<=args.iterations<=10000000 and 1<=args.o0_iterations<=10000000 and 1<=args.python_iterations<=10000000 and 1<=args.repeats<=100):
        parser.error('measurement bounds exceeded')
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil','--example','native_build'],cwd=paired.ROOT,check=True)
    builder=paired.ROOT/'target/release/examples/native_build'
    counters,tokenizers=paired.load_tokenizers()
    manifest=args.manifest;cases=json.loads(manifest.read_text())['cases'];reports=[]
    with tempfile.TemporaryDirectory(prefix='nil-typed-bench-') as temporary:
        folder=Path(temporary)
        for index,case in enumerate(cases):
            name=case['id'];src=CORPUS/'programs'/f'{name}.nil';rt=folder/(name+'.c')
            builds={};binaries={}
            for level in ['O0','O2']:
                binary=folder/(name+'-nil-'+level)
                result=subprocess.run([str(builder),'expr-v4',str(src),'0',str(binary),level,'unbounded',str(rt)],check=True,capture_output=True,text=True,timeout=120)
                builds['nil_'+level]=json.loads(result.stdout);binaries['nil_'+level]=binary
            local=args.baseline=='local' and name.startswith(('reverse-','prefix-'))
            baseline_name=name+'.local' if local else name
            binaries['cpp'],builds['cpp']=cpp_build(folder,case|{'id':baseline_name},rt)
            program=paired.load_program(CORPUS/'programs'/f'{baseline_name}.py')
            for check in case['checks']:
                before=json.dumps(check['args'])
                if program(*check['args'])!=check['expected']: raise ValueError(f'Python oracle mismatch: {name}')
                if json.dumps(check['args'])!=before: raise ValueError('Python mutated caller arguments')
                for label,binary in binaries.items():
                    result=subprocess.run([str(binary),*map(str,flatten(check['args']))],check=True,capture_output=True,text=True,timeout=120)
                    if json.loads(result.stdout)!=check['expected']: raise ValueError(f'{label} oracle mismatch: {name}')
            check=case['checks'][0];samples={name:[] for name in [*binaries,'python']};orders=[]
            for repeat in range(args.repeats):
                order=list(samples);offset=(index+repeat)%len(order);order=order[offset:]+order[:offset];orders.append(order)
                for label in order:
                    sample=(measure_python(program,check['args'],check['expected'],args.python_iterations)
                            if label=='python' else native.native_call(binaries[label],flatten(check['args']),check['expected'],args.o0_iterations if label=='nil_O0' else args.iterations,1)[0])
                    samples[label].append(sample)
            measurements={label:{'runtime_ns_per_call':values,'median_ns_per_call':statistics.median(values),
                                **({'build':builds[label]} if label in builds else {})} for label,values in samples.items()}
            source=source_metrics(case,counters)
            if local:
                source['cpp']=paired.source_measure(CORPUS/'programs'/f'{baseline_name}.cpp',counters)
                source['python']=paired.source_measure(CORPUS/'programs'/f'{baseline_name}.py',counters)
            body=CORPUS/'programs'/f'{name}.body.cpp'
            if body.exists() and not local: source['cpp_body_without_headers_or_helpers']=paired.source_measure(body,counters)
            reports.append({**case,'sources':source,'measurement_orders':orders,'measurements':measurements,
                            'nil_O2_over_cpp':measurements['nil_O2']['median_ns_per_call']/measurements['cpp']['median_ns_per_call'],
                            'nil_O2_over_python':measurements['nil_O2']['median_ns_per_call']/measurements['python']['median_ns_per_call']})
            args.output.parent.mkdir(parents=True,exist_ok=True)
            args.output.with_suffix('.partial.json').write_text(json.dumps({'partial':True,'cases':reports},indent=2)+'\n')
            print('Measured',name,flush=True)
    signatures={
        'numeric_fixed':'(8,i)=a[b]\n(8):8=a[0:1]\n1:b=a>0\n',
        'tagged_fixed':'(a8,i)=a[b]\n(a8):a8=a[0:1]\n1:b=a>0\n',
        'bracket_fixed':'([8],i)=a[b]\n([8]):[8]=a[0:1]\n1:b=a>0\n',
        'full_fixed':'([i64;8],i64)->i64=a[b]\n([i64;8])->[i64;8]=a[0:1]\n1->bool=a>0\n',
    }
    screen={label:{'source':text,'characters':len(text),'bytes':len(text.encode()),'tokens':{k:f(text) for k,f in counters.items()}}
            for label,text in signatures.items()}
    unrolled={}
    def name(i):
        out=''
        while True:
            out=chr(97+i%26)+out;i=i//26-1
            if i<0: return out
    for n in [8,32,128,256]:
        text=f'{n}='+ '+'.join(name(i) for i in range(n))+'\n'
        unrolled[str(n)]={'source':text,'tokens':{k:f(text) for k,f in counters.items()}}
    files=sorted([*list((paired.ROOT/'crates').rglob('*.rs')),*list((paired.ROOT/'cli').rglob('*.rs')),
                  *list(CORPUS.rglob('*.py')),*program_files(CORPUS/'programs'),manifest,Path(__file__),paired.HERE/'native.py',paired.HERE/'run.py'])
    report={'schema':1,'settings':vars(args)|{'output':str(args.output),'manifest':str(args.manifest),'warmup':1000,'profile':'expr-v4','native_instrumentation':'unbounded','seed':None},
            'method':'Complete program source counts exclude shared entry/timing drivers; C++ also reports function-only counts. Same generated C timing driver for NIL/C++; flat typed entry bridges; separate translation units without LTO; NIL O0/O2, C++ O2 -fwrapv. Python timed loop includes result comparison. Selected value/local transform baselines preserve immutable observable value semantics; the baseline setting identifies which is measured. All timed numeric intermediates fit i64; Python general overflow/division semantics are not claimed equivalent. Sorted search inputs are unique. Timings exclude process launch, compilation and input parsing; include marshaling, result comparison and call overhead. Build-stage figures are single samples per level, not compile-time distributions. Signature alternatives are tokenizer screens, not implemented parsers or generation/TCR evidence.',
            'tokenizers':tokenizers,'signature_screen':screen,'v3_unrolled_sum_source_screen':unrolled,
            'clang':subprocess.check_output([os.environ.get('NIL_CLANG','clang'),'--version'],text=True),
            'environment':{'python':paired.platform.python_version(),'platform':paired.platform.platform(),
                'git_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=paired.ROOT,text=True).strip(),
                'git_dirty':bool(subprocess.check_output(['git','status','--porcelain'],cwd=paired.ROOT,text=True).strip())},
            'source_hashes':{str(p.resolve().relative_to(paired.ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
            'cases':reports,'unmeasured':['generation tokens','parse/type/compile rates for model outputs','repair attempts/tokens','TCR'],
            'summary':{'cases':len(reports),'semantic_checks':sum(len(c['checks'])*4 for c in reports),
                       'native_builds':len(reports)*2,'cpp_builds':len(reports),
                       'parity_target':1.25,
                       'worst_nil_over_cpp':max(c['nil_O2_over_cpp'] for c in reports),
                       'parity_failures':[c['id'] for c in reports if c['nil_O2_over_cpp']>1.25]}}
    args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
    args.output.with_suffix('.partial.json').unlink(missing_ok=True)
    print(report['summary']);print('Raw results:',args.output)

if __name__=='__main__': main()
