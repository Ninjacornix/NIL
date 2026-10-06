# External v5 corpus and density evidence

## Finding and coverage

**The favourable Python density result does not hold externally.** NIL costs
10.6–19.9% more source tokens on 31 supported external properties and 5.8–13.6%
more on 55 combined paired tasks. The original 24 reproduce 3.7–8.2% fewer.
All four tokenizers agree on that reversal. C++ totals remain higher, but
external grade-school roster loses to both baselines under every tokenizer.
[All per-task counts, sizes and three cuts](../../benchmarks/reports/2026-10-03/V5_EXTERNAL_DENSITY.md).

75 tasks are retained: 25 self-authored, 50 externally derived. There are
**55 verified NIL programs and 20 unsupported tasks**, including 19 new ones.
Some unsupported algorithms could be emulated; missing map/tree/calendar/dictionary
adapters are distinguished from missing float/entropy/concurrency/reactive contracts.
[Each reason and per-gap counts](../../benchmarks/corpora/application-v5/GENERALITY_GAPS.json).
No baseline implementation or NIL count is fabricated for unsupported new tasks.

## Frozen selection and implementation history

Benchmark commits:

```text
eb90202 docs(corpus): freeze 50 external exercises
7f5f8b5 docs(corpus): freeze external transport contracts
a52c203 feat(corpus): verify external application tasks
```

Fifty alphabetical canonical-data directories in Exercism revision
`9943fd751b684ba1a6a8673c3700a6e79c10a808` were selected before support classification,
solutions and counts. First declared canonical property per exercise; all matching
cases except explicitly superseded UUIDs. This is not full API coverage. Each task
has [provenance/license metadata](../../benchmarks/corpora/application-v5/PROVENANCE.json)
and the upstream MIT notice is retained. Both sides are authored locally.

No task/source was edited after counts. Two corrections before counting were
recorded rather than hidden: Armstrong's 108/127-bit cases make it unsupported;
invalid binary output is null, matching already-frozen expected values. Canonical
cases/expected values were not changed. Initial source-header/bool-equality/C++
escaping mistakes were corrected as authoring errors, not compiler bugs.

Coin-change DP and repeated Hex flood sweeps exceeded unchanged 100,000-step limits;
branch-and-bound and union-find pass the same upstream fixtures. No timeout/fuel/quota
was raised. A host-side 3,125-state basket comparison matches the grouping formula
to independent subset DP; it is additional algorithm evidence, not counted as NIL
execution checks. No production bug was found or fixed.

## Commands and results (exit 0)

```sh
./scripts/ci.sh > /tmp/nil-external/ci.log 2>&1
# 220 passed, 0 failed, strict Clippy clean.
benchmarks/paired/.venv/bin/python -m unittest discover -s benchmarks/corpora/application-v5/tests -v
# 10 tests, OK. Negative original tests deliberately print rejected fixture outputs.
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --group original --output /tmp/nil-external/original-verification.json
# tasks=25, nil_programs=24, unsupported=1
# reference_checks=120, native_checks=240, python_checks=125, cpp_checks=125: 610 total
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/verify.py --output /tmp/nil-external/verification.json
# tasks=75, nil_programs=55, unsupported=20
# reference_checks=474, native_checks=948, python_checks=479, cpp_checks=479: 2380 total
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/density.py --verification /tmp/nil-external/verification.json --output /tmp/nil-external/density
# Adaptation examples: 55
# QWEN_CROSSCHECK comparisons=167, mismatches=[]; tokenizers 0.22.1
benchmarks/paired/.venv/bin/python benchmarks/corpora/application-v5/export.py --verification /tmp/nil-external/verification.json --output /tmp/nil-external/adaptation.jsonl
# Exported 55 verified NIL examples. Byte-identical to density.py export.
```

Native checks divide into 474 O0 and 474 O2. External contribution: 354 reference,
708 native, 354 Python, 354 C++ = 1,770. Upstream expected-value fixture oracles are
finite lookups; unknown input is rejected. They are independent of reference code,
but not algorithms that establish correctness for arbitrary unseen inputs. There
are 353 distinct supported external task/input pairs; Hello World repeats its only
canonical input for the appended golden. Do not equate check count with independence.

The existing in-memory Host, native backend, CLI adapters, quota, 256-call and
100,000-step limits are unchanged. verify.py gained group loading/provenance
fingerprinting and unsupported baseline rows; density.py gained the three cuts;
export.py includes new source provenance. No new execution harness or Python/Rust
runtime dependency was introduced. Formatting new C++ used clang-format 23.1.2;
only new files were formatted.

## Token accounting

Same pinned real vocabularies: Gemma 3 4B and Qwen 2.5 7B vocabulary-only GGUF
via llama-cpp-python 0.3.16; cl100k/o200k via tiktoken 0.12.0. Source bytes include
imports/headers/helpers/I/O/whitespace, excluding only shared CLI adapters. No BOS/EOS,
inference context or model server. [Tokenizer hashes/probes/versions and exact cuts](../../benchmarks/reports/2026-10-03/V5_EXTERNAL_DENSITY.json).

| Cut | G: NIL/Python/C++ | Q: NIL/Python/C++ | cl100k: NIL/Python/C++ | o200k: NIL/Python/C++ |
|---|---:|---:|---:|---:|
| original 24 | 2144/2227/5454 | 1634/1728/4235 | 1582/1705/4217 | 1571/1712/4238 |
| new external 31 | 7372/6147/12712 | 5650/5025/10148 | 5520/4949/10113 | 5498/4972/10153 |
| combined 55 | 9516/8374/18166 | 7284/6753/14383 | 7102/6654/14330 | 7069/6684/14391 |

The full CSV contains 900 task/language/tokenizer rows, including unsupported
blanks. Each original source SHA/byte/character/token count matches the previous
V5_GENERALITY_DENSITY.json exactly. Only supported identical-task pairs enter sums.
File wrappers favour NIL on tiny exercises; library operations and algorithm choices
also affect counts. Neither deterministic counts nor alphabetical sampling yield
representative population confidence. No universal language ranking is supported.

## Immutability and adaptation audit

```sh
git diff 775cd62 -- crates cli tools scripts Cargo.toml Cargo.lock .github docs/language docs/adr
# Empty: compiler/runtime/profiles/default/dependencies untouched.
```

The corpus unit test checks the SHA-256 lock of original tasks.json, oracles.py,
goldens.json and all 74 original sources. Other tests verify source pin/license
integrity, all 50 retained tasks, explicit unsupported reasons, golden metadata
and rejection of unknown external inputs. Final artifact audit confirms current
verification/density fingerprints, 55 matching exports, 900 CSV rows, 2,380 checks,
220 CI tests and provenance/license entries for all 75 tasks.

[Source hashes and full verdicts](../../benchmarks/reports/2026-10-03/V5_EXTERNAL_VERIFICATION.json)
and [separate original rerun](../../benchmarks/reports/2026-10-03/V5_EXTERNAL_ORIGINAL_VERIFICATION.json)
are committed with the immutable source corpus. Unsupported expected values are
retained but never counted as verified solutions.

55 positives are 24 original + 31 external, roughly 9–36× below the earlier
500–2,000 independent-task planning heuristic. API slices and related families
further limit diversity. This is still a seed, not adaptation scale or successful
training evidence. The prior generation result stays NIL 0/24 versus Python 11/24.
Source density is not total tokens until correctness. No model inference/training
or new compiler feature occurred.

## Raw command evidence

Local archive (logs including authoring failures, exact tokenizer/count records,
verification, export and authoring/proof scripts; no model tensors or binaries):

- `/Users/ninjacornix/.local/share/nil/benchmarks/archive/v5-external-density-2026-10-03.zip`
- SHA-256 `293f6e778124818f4c938bc64b4a16681987876bf17ef5472290f2e95a08631c`

All work is committed locally. No compiler or submodule branch was pushed.
