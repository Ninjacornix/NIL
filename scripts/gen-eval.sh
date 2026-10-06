#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -f benchmarks/generation/standing/run.py ]]; then
  echo 'Initialize the local benchmarks branch containing generation/standing.' >&2
  exit 1
fi
exec benchmarks/paired/.venv/bin/python benchmarks/generation/standing/run.py "$@"
