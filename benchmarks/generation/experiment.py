"""Paired local-model generation/repair trials; never execute generated host code."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import re
import statistics
import subprocess
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
PROFILES = ['expr-v0', 'expr-v1', 'expr-v2']


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


def extract_source(text):
    match = re.fullmatch(r'\s*```(?:nil|text)?\s*\n(.*?)\n```\s*', text, re.S)
    return (match.group(1) if match else text).strip() + '\n'


def summary(trials):
    groups = {}
    for trial in trials:
        groups.setdefault((trial['model'], trial['profile']), []).append(trial)
    result = []
    for (model, profile), rows in sorted(groups.items()):
        solved = [r for r in rows if r['solved']]
        usage_known = all(r['output_tokens'] is not None for r in rows)
        spent = sum(r['output_tokens'] for r in rows) if usage_known else None
        inputs = sum(r['input_tokens'] for r in rows) if all(r['input_tokens'] is not None for r in rows) else None
        result.append(dict(model=model, profile=profile, trials=len(rows), solved=len(solved),
            solve_rate=len(solved)/len(rows), first_pass=sum(r['solved'] and r['attempts']==1 for r in rows)/len(rows),
            output_tokens=spent, input_tokens=inputs,
            tcr=spent/len(solved) if solved and usage_known else None,
            tcr_total=(spent+inputs)/len(solved) if solved and spent is not None and inputs is not None else None,
            median_solved_ttCP=statistics.median(r['output_tokens'] for r in solved) if solved and all(r['output_tokens'] is not None for r in solved) else None,
            repair_tokens=sum(r['repair_tokens'] for r in rows) if all(r['repair_tokens'] is not None for r in rows) else None))
    return result


def paired_interval(trials, model, baseline, candidate, repetitions=2000):
    """Bootstrap entire tasks, preserving seeds and paired representations."""
    selected = [r for r in trials if r['model']==model and r['profile'] in [baseline,candidate]]
    by_task = {}
    for r in selected:
        by_task.setdefault(r['task'], []).append(r)
    keys = sorted(by_task)
    if not keys:
        return None
    for rows in by_task.values():
        if {r['seed'] for r in rows if r['profile']==baseline} != {r['seed'] for r in rows if r['profile']==candidate}:
            raise ValueError('unpaired trials')
    rng=random.Random(20261001); ratios=[]; differences=[]
    for _ in range(repetitions):
        rows=[r for key in rng.choices(keys,k=len(keys)) for r in by_task[key]]
        reports={r['profile']:r for r in summary(rows)}
        differences.append(reports[candidate]['solve_rate']-reports[baseline]['solve_rate'])
        a,b=reports[baseline]['tcr'],reports[candidate]['tcr']
        if a is not None and b is not None and a>0:
            ratios.append(b/a)
    def bounds(values):
        values=sorted(values)
        return [values[int(.025*(len(values)-1))],values[int(.975*(len(values)-1))]] if values else None
    return dict(model=model, baseline=baseline, candidate=candidate,
        task_clusters=len(keys), repetitions=repetitions, tcr_ratio_95=bounds(ratios),
        solve_rate_difference_95=bounds(differences), undefined_tcr_resamples=repetitions-len(ratios))


def system_prompt(profile):
    header = {
        'expr-v0': 'Functions: f0(x,y)=expression; calls f1(x,y). Named parameters. Entry is f0.',
        'expr-v1': 'Functions: x,y=expression; calls a(x,y), b(x,y). Functions are a,b,c in line order; entry a. Named parameters.',
        'expr-v2': 'Functions: 2=expression for two parameters a,b. Functions are a,b,c in line order; entry a. A bare a/b is a parameter; a(...)/b(...) calls a function. Omit arity for zero parameters.'
    }[profile]
    loop='@' if profile=='expr-v2' else 'loop'
    example={'expr-v0':'f0(x,y)=x*x+y*y','expr-v1':'x,y=x*x+y*y','expr-v2':'2=a*a+b*b'}[profile]
    demos={
        'expr-v0':'f0(n)=loop(n;a>0;a-1;a)\nf0(x)=f1(x)+1\nf1(y)=y*y',
        'expr-v1':'n=loop(n;a>0;a-1;a)\nx=b(x)+1\ny=y*y',
        'expr-v2':'1=@(a;a>0;a-1;a)\n1=b(a)+1\n1=a*a',
    }[profile]
    countdown,entry,helper=demos.splitlines()
    return f'''Write only a complete NIL program in {profile}; no explanation or markdown.
{header}
One function per line. Parameters/results are signed i64. Literals are decimal.
Operators: + - * /, comparisons < <= > >= == !=, lazy condition?yes:no.
Precedence: calls/parentheses, * /, + -, comparisons, ternary. Comparisons return bool; no integer truthiness. Division truncates toward zero. Overflow and division by zero trap. Unary minus only on numeric literals; use 0-x otherwise.
Example: {example}
Loops: {loop}(initial_values;bool_condition;updated_values;result).
Initial/update values are comma-separated. Condition, updates and result see ONLY loop states a,b,c,... (not outer parameters). Updates read the OLD state and apply simultaneously. Pass outer values as additional initial states if needed. Nested loops have new state scopes. Example {loop}(0,3;b>0;a+2,b-1;a) returns 6.
Complete example program that decrements a nonnegative input to zero:
{countdown}
A separate complete example program computing input squared plus one through a helper:
{entry}
{helper}
Task input names are descriptive; use the parameter names defined in your header (positional a,b,... for expr-v2). Loops always rename their states a,b,..., even when function parameters have other names.
No assignments, declarations, return keyword, logical operators, modulo, arrays, comments, strings, imports or other syntax. Helper functions and recursion are allowed. The entry must accept exactly the task arguments and return the requested i64. Use any correct algorithm within 100000 execution steps and 256 call frames. All specified valid inputs fit i64.'''


def post(endpoint, path, body=None):
    payload=json.dumps(body).encode() if body is not None else None
    request=urllib.request.Request(endpoint+path,data=payload,headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=180) as response:
        return json.load(response)


def evaluate(source, profile, task, folder):
    path=folder/'candidate.nil';path.write_text(source)
    vectors=folder/'vectors.txt';vectors.write_text('\n'.join(' '.join(map(str,c['args'])) for c in task['checks'])+'\n')
    outcome=dict(parse_ok=None,typecheck_ok=None,compile_ok=None,semantic_ok=False,
                 tests_passed=0,tests_total=len(task['checks']),frontend_ns=None,backend_ns=None,runtime_ns=None,binary_bytes=None)
    try:
        result=subprocess.run([str(ROOT/'target/release/examples/generation_check'),profile,str(path),str(vectors)],capture_output=True,text=True,timeout=5)
        lines=result.stdout.splitlines()
        if result.returncode or not lines:
            return outcome|{'failure_reason':'checker process failed','feedback':'Compiler checker failed.'}
        first=lines[0].split('\t',3)
        if first[0]=='ERR':
            parse=first[1]=='Parse'
            return outcome|dict(parse_ok=not parse,typecheck_ok=None if parse else False,compile_ok=None,
                frontend_ns=int(first[2]),diagnostic=first[3],failure_reason='parse' if parse else 'type',feedback=first[3])
        outcome.update(parse_ok=True,typecheck_ok=True,frontend_ns=int(first[1]))
        for line,check in zip(lines[1:],task['checks']):
            fields=line.split('\t',3)
            if fields[0]=='VALUE' and int(fields[2])==check['expected']:
                outcome['tests_passed']+=1
        if len(lines)-1!=len(task['checks']):
            outcome['tests_passed']=0
        # Safe native pipeline: NIL has no I/O, FFI or memory instructions here.
        binary=folder/'candidate-bin'
        start=time.perf_counter_ns()
        built=subprocess.run([str(ROOT/'target/release/nil'),'--profile',profile,'--bounded','build',str(path),'-o',str(binary)],capture_output=True,text=True,timeout=15)
        outcome['build_wall_ns']=time.perf_counter_ns()-start
        outcome['compile_ok']=built.returncode==0
        if built.returncode:
            return outcome|dict(failure_reason='backend',feedback='Native compilation failed: '+built.stderr[:1000])
        outcome['binary_bytes']=binary.stat().st_size
        native_passed=0
        for check in task['checks']:
            run=subprocess.run([str(binary),*map(str,check['args'])],capture_output=True,text=True,timeout=2)
            if run.returncode==0 and run.stdout.strip()==str(check['expected']):
                native_passed+=1
        outcome['native_tests_passed']=native_passed
        outcome['semantic_ok']=outcome['tests_passed']==len(task['checks']) and native_passed==len(task['checks'])
        outcome['feedback']='Correct.' if outcome['semantic_ok'] else 'Semantic tests failed. Check the task contract, edge cases, scope and simultaneous loop updates; replace the entire program.'
        outcome['failure_reason']=None if outcome['semantic_ok'] else 'semantic'
    except subprocess.TimeoutExpired:
        outcome.update(failure_reason='timeout',feedback='Execution/compilation timed out; use a terminating algorithm within the stated limits.')
    return outcome


def run(args):
    tasks=json.loads((HERE/'tasks.json').read_text())['tasks']
    if args.task_limit: tasks=tasks[:args.task_limit]
    args.output.mkdir(parents=True,exist_ok=True)
    if (args.output/'attempts.jsonl').exists(): raise ValueError('output already contains attempts; choose a fresh directory')
    tags=post(args.endpoint,'/api/tags')['models']
    models={m['name']:m for m in tags}
    for name in args.models:
        if name not in models: raise ValueError('model not installed: '+name)
    files=[Path(__file__),HERE/'tasks.json',ROOT/'crates/nil-compiler/examples/generation_check.rs',ROOT/'Cargo.toml',ROOT/'Cargo.lock',*list((ROOT/'crates').rglob('*.rs')),*list((ROOT/'cli').rglob('*.rs'))]
    config=dict(schema=1,models={name:models[name] for name in args.models},server_version=post(args.endpoint,'/api/version'),
        model_details={name:post(args.endpoint,'/api/show',{'model':name}) for name in args.models},
        seeds=args.seeds,profiles=PROFILES,temperature=.2,attempt_limit=3,output_budget=640,per_attempt_limit=256,
        num_ctx=4096,tasks=[t['id'] for t in tasks],correctness_margin=.05,tcr_improvement=.10,
        source_hashes={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
        git_revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),git_dirty=True,
        method='Paired fixed checked-i64 semantics. Reference and bounded native oracles. No hidden vectors or solutions in prompts/repair feedback. eval_count counts all returned output; reasoning is not separately tokenized or added. Source tokens are independent cl100k/Qwen screens. No automatic infrastructure retries. Few-shot adaptation uses fixed countdown and helper examples, not task solutions. Exploratory tasks may overlap known examples; not contamination-proof. Profile order rotates by task and seed. Models run sequentially to avoid memory pressure.')
    (args.output/'config.json').write_text(json.dumps(config,indent=2)+'\n')
    import sys
    sys.path.insert(0,str(ROOT/'benchmarks/paired'))
    import run as paired
    counters,tokenizers=paired.load_tokenizers()
    config['source_tokenizers']=tokenizers
    (args.output/'config.json').write_text(json.dumps(config,indent=2)+'\n')
    trials=[]
    with (args.output/'attempts.jsonl').open('x') as log:
        for model in args.models:
            for index,task in enumerate(tasks):
                for seed in args.seeds:
                    offset=(index+seed)%3
                    for profile in PROFILES[offset:]+PROFILES[:offset]:
                        trial_id=f'{model.replace(":","-").replace("/","-")}-{task["id"]}-{seed}-{profile}'
                        folder=args.output/trial_id;folder.mkdir()
                        instruction=system_prompt(profile)+'\nTask: '+task['prompt']
                        messages=[{'role':'user','content':instruction}]
                        output_tokens=0;input_tokens=0;repair_tokens=0;solved=False;usage_known=True;input_known=True
                        for attempt in range(3):
                            remaining=640-output_tokens
                            if remaining<=0: break
                            request=dict(model=model,messages=messages,stream=False,keep_alive='5m',options=dict(seed=seed,temperature=.2,num_predict=min(256,remaining),num_ctx=4096))
                            (folder/f'request-{attempt}.json').write_text(json.dumps(request,indent=2))
                            start=time.perf_counter_ns()
                            try:
                                response=post(args.endpoint,'/api/chat',request)
                            except Exception as exc:
                                record=dict(trial_id=trial_id,attempt=attempt,model=model,profile=profile,task=task['id'],seed=seed,
                                    generated_tokens=None,input_tokens=None,semantic_ok=False,failure_reason='infrastructure',error=str(exc))
                                log.write(json.dumps(record)+'\n');log.flush();usage_known=False;input_known=False;break
                            raw=json.dumps(response,indent=2);(folder/f'response-{attempt}.json').write_text(raw)
                            text=response['message']['content'];source=extract_source(text)
                            (folder/f'source-{attempt}.nil').write_text(source)
                            verdict=evaluate(source,profile,task,folder)
                            used=response.get('eval_count');inputs=response.get('prompt_eval_count')
                            if not isinstance(used,int) or used<0: usage_known=False
                            else: output_tokens+=used;repair_tokens+=used if attempt else 0
                            if not isinstance(inputs,int) or inputs<0: input_known=False
                            else: input_tokens+=inputs
                            record=dict(trial_id=trial_id,model=model,profile=profile,task=task['id'],seed=seed,attempt=attempt,
                                prompt_sha256=digest(json.dumps(messages,sort_keys=True)),test_sha256=digest(json.dumps(task['checks'],sort_keys=True)),
                                source_sha256=digest(source),source_characters=len(source),source_bytes=len(source.encode()),
                                source_tokens={name:counter(source) for name,counter in counters.items()},generated_tokens=used,input_tokens=inputs,
                                reasoning_tokens=None,thinking_text_present=bool(response['message'].get('thinking')),repair_tokens=used if attempt else 0,
                                cumulative_output_tokens=output_tokens if usage_known else None,wall_ns=time.perf_counter_ns()-start,
                                done_reason=response.get('done_reason'),**verdict)
                            log.write(json.dumps(record)+'\n');log.flush()
                            print(trial_id,attempt,used,verdict['failure_reason'] or 'correct',flush=True)
                            solved=verdict['semantic_ok']
                            if solved or not usage_known: break
                            messages.extend([{'role':'assistant','content':text},{'role':'user','content':verdict['feedback']+' Return the entire corrected NIL program, one function per line. Loop regions use only positional states a,b,c,...; function parameters are not captured.'}])
                        trial=dict(model=model,profile=profile,task=task['id'],seed=seed,solved=solved,attempts=attempt+1,
                            output_tokens=output_tokens if usage_known else None,input_tokens=input_tokens if input_known else None,
                            repair_tokens=repair_tokens if usage_known else None)
                        trials.append(trial)
                        with (args.output/'trials.jsonl').open('a') as file: file.write(json.dumps(trial)+'\n')
                        (args.output/'summary.json').write_text(json.dumps(summary(trials),indent=2)+'\n')
    intervals=[paired_interval(trials,m,'expr-v0',p) for m in args.models for p in PROFILES[1:]]
    (args.output/'paired-intervals.json').write_text(json.dumps(intervals,indent=2)+'\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--endpoint',default='http://127.0.0.1:11434')
    parser.add_argument('--models',nargs='+',default=['gemma3:4b','qwen2.5:7b-instruct'])
    parser.add_argument('--seeds',nargs='+',type=int,default=[17,29])
    parser.add_argument('--task-limit',type=int)
    parser.add_argument('--output',type=Path,required=True)
    run(parser.parse_args())
