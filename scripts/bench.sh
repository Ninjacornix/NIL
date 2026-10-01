#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -f benchmarks/bench.py ]]; then
  echo "Initialize the benchmark suite: git submodule update --init benchmarks" >&2
  exit 2
fi
export NIL_ROOT="$PWD"
if [[ -x benchmarks/paired/.venv/bin/python ]]; then
  exec benchmarks/paired/.venv/bin/python benchmarks/bench.py "$@"
fi
exec python3 benchmarks/bench.py "$@"
