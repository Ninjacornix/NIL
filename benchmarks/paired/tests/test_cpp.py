import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cpp


class CppComparisonTests(unittest.TestCase):
    def test_shared_runtime_has_exact_entry_arity_and_no_markers(self):
        runtime = cpp.driver(3)
        self.assertNotIn('$', runtime)
        self.assertIn('uint64_t, uint64_t, int64_t, int64_t, int64_t', runtime)
        self.assertIn('values[0], values[1], values[2]', runtime)
        self.assertIn('UINT64_C(100000)', runtime)

    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='nil-cpp-tests-')
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.folder = Path(cls.temporary.name)
        cls.probes = {}
        arities = [1, 1, 2, 1, 1, 3, 2, 1]
        clang = os.environ.get('NIL_CLANG', 'clang')
        for name, arity in zip(cpp.CASE_IDS, arities):
            binary, _ = cpp.build_cpp(cls.folder, name, arity, True)
            # Probe the same object directly to verify remaining fuel/depth, not timing.
            params = ', int64_t' * arity
            values = ''.join(f', strtoimax(argv[{i+3}],0,10)' for i in range(arity))
            probe = cls.folder / f'{name}-probe.c'
            probe.write_text(f'''#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
typedef struct {{ uint64_t fuel,depth,limit; }} Context;
extern int64_t nil_fn0(Context*,uint64_t,uint64_t{params});
_Noreturn void nil_fail(uint32_t reason,uint64_t start,uint64_t end) {{
 fprintf(stderr,"reason:%u",reason);exit(1);
}}
int main(int argc,char** argv) {{
 Context c={{strtoumax(argv[1],0,10),0,strtoumax(argv[2],0,10)}};
 int64_t r=nil_fn0(&c,UINT64_MAX,UINT64_MAX{values});
 printf("%" PRId64 " %" PRIu64 " %" PRIu64,r,c.fuel,c.depth);
}}
''')
            executable = cls.folder / f'{name}-probe'
            subprocess.run([clang, '-O2', '-std=c11', str(probe),
                            str(cls.folder / f'{name}-checked.o'), '-o', str(executable)],
                           check=True, capture_output=True, text=True)
            cls.probes[name] = executable

    def probe(self, name, fuel, depth, arguments):
        return subprocess.run([str(self.probes[name]), str(fuel), str(depth), *map(str, arguments)],
                              capture_output=True, text=True, timeout=10)

    def test_checked_cpp_matches_hir_fuel_and_restores_depth(self):
        # Counts include constants, operations, loop/if instructions, region yields and return.
        cases = [('factorial', [10], 3628800, 70), ('fibonacci', [20], 6765, 145),
                 ('max', [20, 42], 42, 4), ('counted_sum', [100], 5050, 707),
                 ('absolute', [-5], 5, 7), ('clamp', [12, 0, 10], 10, 7),
                 ('gcd', [1071, 462], 21, 27), ('nested_sum', [10], 55, 577)]
        for name, args, expected, fuel in cases:
            with self.subTest(name=name):
                result = self.probe(name, fuel, 1, args)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, f'{expected} 0 0')
                exhausted = self.probe(name, fuel-1, 1, args)
                self.assertEqual(exhausted.returncode, 1)
                self.assertEqual(exhausted.stderr, 'reason:0')
                depth = self.probe(name, fuel, 0, args)
                self.assertEqual(depth.returncode, 1)
                self.assertEqual(depth.stderr, 'reason:1')

    def test_checked_cpp_traps_at_signed_boundaries(self):
        for name, args in [('factorial', [21]), ('fibonacci', [93]),
                           ('absolute', [-(2**63)]), ('gcd', [-(2**63), -1])]:
            with self.subTest(name=name):
                result = self.probe(name, 100000, 256, args)
                self.assertEqual(result.returncode, 1)
                self.assertEqual(result.stderr, 'reason:2')
