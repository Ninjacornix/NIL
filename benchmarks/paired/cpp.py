"""Compare NIL LLVM O2 with ordinary and semantically checked C++17/Clang O2."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import statistics
import subprocess
import tempfile
import time
from pathlib import Path
import native
import run as paired

CASE_IDS = ['factorial', 'fibonacci', 'max', 'counted_sum', 'absolute', 'clamp', 'gcd', 'nested_sum']
CPP = paired.HERE / 'cpp/program.cpp'
RUNTIME = paired.ROOT / 'crates/nil-llvm/src/runtime.rs'


def driver(arity: int, steps: int = 100000, depth: int = 256) -> str:
    """Use the compiler's actual C driver so timing and entry ABI are identical."""
    text = RUNTIME.read_text().split('const RUNTIME: &str = r#"', 1)[1].rsplit('"#;', 1)[0]
    replacements = {
        '$ENTRY': '0', '$PARAMETERS': ', int64_t' * arity,
        '$ARGUMENTS': ''.join(f', values[{i}]' for i in range(arity)),
        '$ARITY': str(arity), '$STEPS': str(steps), '$DEPTH': str(depth),
    }
    for key, value in replacements.items():
        text = text.replace(key, value)
    if '$' in text:
        raise ValueError('unresolved runtime template marker')
    return text


def invoke(command: list[str]) -> float:
    start = time.perf_counter_ns()
    subprocess.run(command, check=True, capture_output=True, text=True, timeout=120)
    return (time.perf_counter_ns() - start) / 1e6


def build_cpp(folder: Path, name: str, arity: int, checked: bool,
              steps: int = 100000, depth: int = 256) -> tuple[Path, dict]:
    clang = os.environ.get('NIL_CLANG', 'clang')
    tag = name + ('-checked' if checked else '-ordinary')
    support = folder / (tag + '.c')
    support.write_text(driver(arity, steps, depth))
    obj = folder / (tag + '.o'); runtime_obj = folder / (tag + '-runtime.o')
    binary = folder / tag
    flags = ['-DNIL_CHECKED'] if checked else []
    program_source = CPP
    if not checked:
        program_source = folder / (tag + '.cpp')
        parameters = ''.join(f', std::int64_t p{i}' for i in range(arity))
        arguments = ','.join(f'p{i}' for i in range(arity))
        ordinary = paired.HERE / f'cpp/sources/{name}.cpp'
        program_source.write_text(ordinary.read_text() + f'extern "C" std::int64_t nil_fn0(void*,std::uint64_t,std::uint64_t{parameters}){{return program({arguments});}}\n')
    compile_ms = invoke([clang, '-O2', '-std=c++17', '-x', 'c++',
                         f'-DCASE={CASE_IDS.index(name)+1}', *flags, '-c', str(program_source), '-o', str(obj)])
    runtime_ms = invoke([clang, '-O2', '-std=c11', '-x', 'c', '-c', str(support), '-o', str(runtime_obj)])
    link_ms = invoke([clang, str(obj), str(runtime_obj), '-o', str(binary)])
    return binary, {'cpp_compile_ms': compile_ms, 'runtime_compile_ms': runtime_ms,
                    'link_ms': link_ms, 'total_ms': compile_ms+runtime_ms+link_ms,
                    'binary_bytes': binary.stat().st_size,
                    'runtime_sha256': hashlib.sha256(support.read_bytes()).hexdigest()}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--nil-profile', choices=['expr-v2','expr-v3'], default='expr-v2')
    parser.add_argument('--nil-instrumentation', choices=['auto','bounded','unbounded'], default='auto')
    parser.add_argument('--iterations', type=int, default=1000000)
    parser.add_argument('--repeats', type=int, default=9)
    parser.add_argument('--stress', action='store_true', help='time larger valid inputs as well as checking the full oracle suite')
    args = parser.parse_args()
    if not (1 <= args.iterations <= 10000000 and 1 <= args.repeats <= 100):
        parser.error('measurement bounds exceeded')
    subprocess.run(['cargo', 'build', '--release', '--locked', '--offline', '-p', 'nil',
                    '--example', 'native_build'], cwd=paired.ROOT, check=True)
    builder = paired.ROOT / 'target/release/examples/native_build'
    manifest_path = paired.HERE / 'cases-control-v2.json'
    manifest = json.loads(manifest_path.read_text()); paired.validate_cases(manifest['cases'])
    counters, tokenizers = paired.load_tokenizers()
    stress_args = {'factorial': [20], 'fibonacci': [92], 'max': [-(2**63),2**63-1],
                   'counted_sum': [1000], 'absolute': [-(2**63-1)], 'clamp': [5,0,10],
                   'gcd': [1836311903,1134903170], 'nested_sum': [40]}
    cases = []
    with tempfile.TemporaryDirectory(prefix='nil-cpp-bench-') as temporary:
        folder = Path(temporary)
        for index, case in enumerate(manifest['cases']):
            source = paired.HERE / case['nil']; arity = len(case['checks'][0]['args'])
            nil_binary = folder / (case['id'] + '-nil')
            built = subprocess.run([str(builder), args.nil_profile, str(source),
                        str(case['function']), str(nil_binary), 'O2', args.nil_instrumentation], check=True,
                        capture_output=True, text=True, timeout=120)
            binaries = {'nil': nil_binary}; builds = {'nil': json.loads(built.stdout)}
            for checked, label in [(False, 'cpp_ordinary'), (True, 'cpp_checked')]:
                binaries[label], builds[label] = build_cpp(folder, case['id'], arity, checked)
            oracle = paired.load_program(paired.HERE / case['python'])
            timed = case['checks'][0]
            checks = list(case['checks'])
            if args.stress:
                timed_args = stress_args[case['id']]
                timed = {'args': timed_args, 'expected': oracle(*timed_args)}
                if timed not in checks:
                    checks.append(timed)
            for check in checks:
                if oracle(*check['args']) != check['expected']:
                    raise ValueError('Python differs from mathematical reference')
                for binary in binaries.values():
                    out = subprocess.run([str(binary), *map(str, check['args'])],
                                check=True, capture_output=True, text=True, timeout=120)
                    if int(out.stdout) != check['expected']:
                        raise ValueError(f"incorrect result for {case['id']}: {binary}")
            samples = {label: [] for label in binaries}; orders = []
            for repeat in range(args.repeats):
                order = list(binaries); offset = (index+repeat) % len(order)
                order = order[offset:] + order[:offset]; orders.append(order)
                for label in order:
                    samples[label].extend(native.native_call(binaries[label], timed['args'],
                                                            timed['expected'], args.iterations, 1))
            measurements = {label: {'runtime_ns_per_call': values,
                             'median_ns_per_call': statistics.median(values), 'build': builds[label]}
                            for label, values in samples.items()}
            cases.append({'id': case['id'], 'timed_args': timed['args'], 'checks': checks, 'timed_expected': timed['expected'],
                          'measurement_orders': orders, 'measurements': measurements,
                          'source': {'nil': paired.source_measure(source,counters),
                                     'cpp': paired.source_measure(paired.HERE/f"cpp/sources/{case['id']}.cpp",counters)}})
            print('Measured', case['id'], flush=True)
    compiler_sources = sorted(list((paired.ROOT/'crates').rglob('*.rs')) + list((paired.ROOT/'cli').rglob('*.rs')))
    files = [*compiler_sources, CPP, *sorted((paired.HERE/'cpp/sources').glob('*.cpp')), Path(__file__), paired.HERE/'native.py', paired.HERE/'run.py',
             manifest_path, *[paired.HERE/c['nil'] for c in manifest['cases']],
             *[paired.HERE/c['python'] for c in manifest['cases']]]
    result = {'schema': 1, 'settings': {'nil_profile': args.nil_profile, 'nil_instrumentation': args.nil_instrumentation, 'stress': args.stress, 'warmup': 1000, 'iterations': args.iterations, 'repeats': args.repeats},
        'method': 'Identical entry ABI and C timing driver, separate translation units, no LTO; O2 for NIL and C++; ordinary C++ omits traps/budgets, checked C++ mirrors v2 HIR instruction/yield fuel and arithmetic checks; times exclude compilation/process launch.',
        'tokenizers': tokenizers,
        'clang': subprocess.check_output([os.environ.get('NIL_CLANG', 'clang'), '--version'], text=True),
        'environment': {'platform': paired.platform.platform(),
            'git_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=paired.ROOT, text=True).strip(),
            'git_dirty': bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=paired.ROOT, text=True).strip())},
        'source_hashes': {str(p.resolve().relative_to(paired.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files},
        'cases': cases}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print('Raw results:', args.output)


if __name__ == '__main__':
    main()
