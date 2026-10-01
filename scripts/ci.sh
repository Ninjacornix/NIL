#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

format() { cargo fmt --all -- --check; }
clippy() { cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings; }
build() { cargo build --workspace --all-targets --all-features --locked --offline; }
tests() {
    cargo test --workspace --lib --bins --tests --all-features --locked --offline
    cargo test --workspace --doc --all-features --locked --offline
}
release() {
    cargo build --workspace --release --all-targets --all-features --locked --offline
    cargo test --workspace --release --lib --bins --tests --all-features --locked --offline
    cargo test --workspace --release --doc --all-features --locked --offline
}
case "${1:-all}" in
    all) format; clippy; build; tests ;;
    format) format ;;
    clippy) clippy ;;
    build) build ;;
    test) tests ;;
    release) release ;;
    *) echo "Usage: $0 [all|format|clippy|build|test|release]" >&2; exit 2 ;;
esac
