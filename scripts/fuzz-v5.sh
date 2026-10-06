#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p nil-fuzz --bin v5 --release --locked --offline
exec target/release/v5 "$@"
