"""Contributor benchmark entry point. Smoke/mock checks need no model downloads."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest
import uuid

HERE=Path(__file__).resolve().parent
ROOT=HERE.parent
sys.path.insert(0,str(HERE/'paired'))


def output_directory(kind, requested=None):
    home=Path(os.environ.get('NIL_BENCH_HOME',Path.home()/'.local/share/nil/benchmarks')).expanduser()
    path=(requested or home/'runs'/(datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')+'-'+kind+'-'+uuid.uuid4().hex[:8])).expanduser().resolve()
    if path==ROOT or ROOT in path.parents:
        raise ValueError('Benchmark output must be outside the repository; use --output or NIL_BENCH_HOME.')
    path.mkdir(parents=True,exist_ok=False)
    return path


def provenance():
    paths=sorted(set([*ROOT.glob('Cargo*'),*HERE.rglob('*.py'),*HERE.rglob('*.json'),
        *HERE.rglob('*.nil'),*HERE.rglob('*.cpp'),*HERE.rglob('*.rs'),*HERE.rglob('*.toml'),
        *HERE.rglob('*.lock'),*list((ROOT/'crates').rglob('*.rs')),*list((ROOT/'cli').rglob('*.rs'))]))
    paths=[p for p in paths if not any(part in ['.venv','.cache','__pycache__','results'] for part in p.parts)]
    return dict(git_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        git_dirty=bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()),
        python=sys.version,platform=platform.platform(),
        source_hashes={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in paths})


def run_tests(paths, names=None):
    suites=[]
    for path in paths:
        suites.append(unittest.defaultTestLoader.discover(str(path),pattern='test_*.py'))
    suite=unittest.TestSuite(suites)
    if names:
        def flatten(suite):
            for test in suite:
                if isinstance(test,unittest.TestSuite): yield from flatten(test)
                else: yield test
        suite=unittest.TestSuite(test for test in flatten(suite) if type(test).__name__ in names)
    if not suite.countTestCases(): raise ValueError('No smoke tests selected')
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    if not result.wasSuccessful(): raise RuntimeError('Benchmark tests failed')
    return dict(tests=result.testsRun,failures=0)


def token_smoke():
    sys.path.insert(0,str(HERE/'paired/tests'))
    return run_tests([HERE/'paired/tests'],{'PairedRunnerTests','ExperimentTests'})|dict(
        method='Offline accounting/asset-validation contracts with test counters; not model tokenizer measurements.')


def token_full():
    import run as paired
    counters,metadata=paired.load_tokenizers()
    import typed
    cases=[]
    for manifest in ['cases.json','heldout.json']:
        for case in json.loads((typed.CORPUS/manifest).read_text())['cases']:
            name=case['id'];baseline=name+'.local' if name.startswith(('reverse-','prefix-')) else name
            cases.append(dict(id=name,sources={language:paired.source_measure(typed.CORPUS/'programs'/f'{source}.{ext}',counters)
                for language,source,ext in [('nil',name,'nil'),('python',baseline,'py'),('cpp',baseline,'cpp')]}))
    return dict(tokenizers=metadata,cases=cases,method='Full source counts, no framing/drivers. Fresh-local transforms; C++ includes headers/helpers. Source tokens do not measure generation/repair efficiency.')


def runtime_smoke(folder):
    import typed
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil','--example','native_build'],cwd=ROOT,check=True)
    builder=ROOT/'target/release/examples/native_build';cases=[]
    manifest=json.loads((typed.CORPUS/'cases.json').read_text())['cases']
    with tempfile.TemporaryDirectory(dir=folder) as directory:
        work=Path(directory)
        for case in manifest:
            if case['id'] not in ['sum-8','reverse-8','predicate']:continue
            runtime=work/'runtime.c';binary=work/'nil-program'
            built=subprocess.run([str(builder),'expr-v4',str(typed.CORPUS/'programs'/f"{case['id']}.nil"),'0',str(binary),'O2','bounded',str(runtime)],capture_output=True,text=True,check=True,timeout=120)
            cpp,stats=typed.cpp_build(work,case,runtime)
            oracle=typed.paired.load_program(typed.CORPUS/'programs'/f"{case['id']}.py")
            for check in case['checks']:
                assert oracle(*check['args'])==check['expected'], 'Python mismatch'
                for implementation in [binary,cpp]:
                    run=subprocess.run([str(implementation),*map(str,typed.flatten(check['args']))],capture_output=True,text=True,check=True,timeout=5)
                    if json.loads(run.stdout)!=check['expected']:raise ValueError(f"Oracle mismatch: {case['id']}")
            cases.append(dict(id=case['id'],checks=len(case['checks']),nil_build=json.loads(built.stdout),cpp_build=stats))
    if len(cases)!=3: raise ValueError('Incomplete smoke fixtures')
    return dict(cases=cases,semantic_comparisons=sum(c['checks'] for c in cases)*3,
                method='NIL/C++/Python correctness only; single build samples are not a speed gate.')


def generation_mock():
    sys.path.insert(0,str(HERE/'generation'))
    return run_tests([HERE/'generation/tests'],{'Metrics','Trajectories','Audit'})|dict(method='Offline mocked provider usage; no inference or model efficiency claim.')


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('kind',choices=['token','runtime','generation'])
    mode=parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--smoke',action='store_true');mode.add_argument('--full',action='store_true')
    mode.add_argument('--mock',action='store_true');mode.add_argument('--ollama',metavar='MODEL')
    parser.add_argument('--output',type=Path);parser.add_argument('--iterations',type=int,default=1000000)
    parser.add_argument('--repeats',type=int,default=9)
    args=parser.parse_args(argv)
    if args.kind!='generation' and (args.mock or args.ollama):parser.error('--mock/--ollama require generation')
    if args.kind=='generation' and (args.smoke or args.full):parser.error('Use generation --mock or --ollama MODEL')
    if not 1<=args.iterations<=10000000 or not 1<=args.repeats<=100:parser.error('Invalid iterations/repeats')
    folder=output_directory(args.kind,args.output)
    record=dict(schema=1,kind=args.kind,mode='smoke' if args.smoke else 'mock' if args.mock else 'ollama' if args.ollama else 'full',settings=vars(args)|{'output':str(folder)},provenance=provenance(),status='running')
    (folder/'manifest.json').write_text(json.dumps(record,indent=2)+'\n')
    print('Benchmark artifacts:',folder,flush=True)
    try:
        if args.kind=='token': result=token_smoke() if args.smoke else token_full()
        elif args.kind=='runtime' and args.smoke: result=runtime_smoke(folder)
        elif args.kind=='runtime':
            for name in ['cases','heldout']:
                subprocess.run([sys.executable,str(HERE/'paired/typed.py'),'--baseline','local','--manifest',str(HERE/f'paired/typed/{name}.json'),'--output',str(folder/f'{name}.json'),'--iterations',str(args.iterations),'--repeats',str(args.repeats)],cwd=ROOT,check=True)
            result=dict(files=['cases.json','heldout.json'])
        elif args.mock: result=generation_mock()
        else:
            subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil-compiler','--example','generation_check','-p','nil'],cwd=ROOT,check=True)
            subprocess.run([sys.executable,str(HERE/'generation/experiment.py'),'--models',args.ollama,'--output',str(folder/'trials')],cwd=ROOT,check=True)
            result=dict(directory='trials')
        record.update(status='passed',result=result)
    except Exception as exc:
        record.update(status='failed',error=str(exc));raise
    finally:
        (folder/'manifest.json').write_text(json.dumps(record,indent=2)+'\n')


if __name__=='__main__':
    try:main()
    except (ImportError,ValueError) as error:
        raise SystemExit(str(error)+'\nFor full tokenizer runs: uv sync --project benchmarks/paired --locked')
