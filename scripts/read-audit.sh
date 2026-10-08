#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $(uname -s) != Darwin ]]; then
  echo 'The full FileHost fault audit requires macOS; run it on the mandatory macOS gate.' >&2
  exit 1
fi
if [[ ! -f benchmarks/core-memory-study/audit_read_order.py ]]; then
  echo 'Initialize benchmarks with git submodule update --init benchmarks.' >&2
  exit 1
fi
cargo build --release --locked --offline -p nil -p nil-compiler -p nil-hir
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s benchmarks/core-memory-study -p 'test_*.py' -v
exec python3 benchmarks/core-memory-study/audit_read_order.py --require-parity "$@"
