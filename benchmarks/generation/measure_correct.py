"""Measure accepted generated programs after inference, using the existing native driver."""
import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys
import tempfile
import experiment as e
from report import audit


def main(root):
    config,trials,attempts=audit(root)
    sys.path.insert(0,str(e.ROOT/'benchmarks/paired'))
    from native import native_call
    tasks={t['id']:t for t in json.loads((e.HERE/'tasks.json').read_text())['tasks']}
    subprocess.run(['cargo','build','--release','--locked','--offline','-p','nil','--example','native_build'],cwd=e.ROOT,check=True)
    accepted=[a for a in attempts if a['semantic_ok']]
    results=[]
    with tempfile.TemporaryDirectory(prefix='nil-generation-measure-') as directory:
        for attempt in accepted:
            source=root/attempt['trial_id']/f"source-{attempt['attempt']}.nil"
            binary=Path(directory)/'program'
            built=subprocess.run([str(e.ROOT/'target/release/examples/native_build'),attempt['profile'],str(source),'0',str(binary),'O2','bounded'],capture_output=True,text=True,check=True,timeout=60)
            check=tasks[attempt['task']]['checks'][0]
            samples=native_call(binary,check['args'],check['expected'],10000,5)
            results.append(dict(trial_id=attempt['trial_id'],source_sha256=attempt['source_sha256'],
                profile=attempt['profile'],model=attempt['model'],task=attempt['task'],timed_input=check,
                build=json.loads(built.stdout),runtime_ns_samples=samples,runtime_ns_median=statistics.median(samples)))
    report=dict(schema=1,settings=dict(optimization='O2',instrumentation='bounded',iterations=10000,repeats=5,warmup=1000),
        method='Only first-correct accepted sources; failures remain in generation/TCR reports. Existing native in-process driver excludes launch/argv/compilation and checks results. Five repeated samples per source; backend stage measurements are single samples. Models should be unloaded before timing. No C++ speed conclusion follows from this pilot.',
        source_hashes={str(Path(__file__).relative_to(e.ROOT)):hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        accepted_count=len(accepted),cases=results)
    (root/'accepted-runtime.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Measured accepted programs:',len(results))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('run',type=Path)
    main(parser.parse_args().run)
