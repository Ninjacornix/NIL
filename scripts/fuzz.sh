#!/usr/bin/env bash
# Reproducible dependency-free mutation/property fuzzing and native differential checks.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p nil-fuzz --locked --offline
exec ./target/release/nil-fuzz "$@"
