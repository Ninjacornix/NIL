#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# nil-llvm invokes NIL_CLANG for both the emitted IR and its C runtime.
# Instrument both compilation and linking without modifying production options.
san_clang=$(command -v "${NIL_CLANG:-clang}")
san_dir=$(mktemp -d "${TMPDIR:-/tmp}/nil-sanitize.XXXXXX")
trap 'rm -rf "$san_dir"' EXIT
export NIL_SANITIZER_CLANG="$san_clang"
cat > "$san_dir/clang" <<'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
exec "$NIL_SANITIZER_CLANG" -fsanitize=address,undefined,float-cast-overflow -fno-sanitize-recover=all -fno-omit-frame-pointer "$@"
WRAPPER
chmod +x "$san_dir/clang"
export NIL_CLANG="$san_dir/clang"
# LeakSanitizer is unavailable on macOS. The arena is released on success;
# failing programs exit immediately. This checks invalid accesses and UB, not leaks.
export ASAN_OPTIONS="${ASAN_OPTIONS:+$ASAN_OPTIONS:}detect_leaks=0:halt_on_error=1"
export UBSAN_OPTIONS="${UBSAN_OPTIONS:+$UBSAN_OPTIONS:}halt_on_error=1:print_stacktrace=1"
# Round 27 P2: targets are discovered, never hand listed, so a new test file is
# covered the day it lands. Only the tests that contend for process-wide
# resources stay serial; everything else runs four-way.
serial="stack memory_guard"
parallel=""
for file in crates/nil-llvm/tests/*.rs; do
  name=$(basename "$file" .rs)
  case " $serial " in *" $name "*) continue ;; esac
  parallel="$parallel --test $name"
done
# shellcheck disable=SC2086
cargo test -p nil-llvm $parallel --locked --offline -- --test-threads=4
for name in $serial; do
  cargo test -p nil-llvm --test "$name" --locked --offline -- --test-threads=1
done
