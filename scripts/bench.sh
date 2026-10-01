#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -x benchmarks/paired/.venv/bin/python ]]; then
  exec benchmarks/paired/.venv/bin/python benchmarks/bench.py "$@"
fi
exec python3 benchmarks/bench.py "$@"
