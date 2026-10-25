#!/usr/bin/env bash
# Round 27 P7: one entry point for the gate suite. See scripts/gates.py.
set -euo pipefail
cd "$(dirname "$0")/.."
# Gates measure prebuilt artifacts, so build once here and never from Python.
cargo build --release -p nil --bin nil --locked --offline
cargo build -p nil-fuzz --bin v5 --release --locked --offline
python3 -c "import sys;sys.path.insert(0,'benchmarks');import support;support.build_helpers('core_check','application_check')"
export NIL_PREBUILT=1
exec python3 scripts/gates.py "$@"
