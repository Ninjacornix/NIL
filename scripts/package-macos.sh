#!/usr/bin/env bash
# Build a standalone CLI archive. Run on macOS with the Rust target installed.
set -euo pipefail
cd "$(dirname "$0")/.."
release_target=${1:?Usage: package-macos.sh TARGET}
case "$release_target" in
  aarch64-apple-darwin|x86_64-apple-darwin) ;;
  *) echo "Unsupported macOS target: $release_target" >&2; exit 2 ;;
esac
[[ $(uname -s) == Darwin ]] || { echo 'Packaging requires macOS.' >&2; exit 2; }
package_id=$(cargo pkgid -p nil --locked --offline)
release_version=${package_id##*#}
release_version=${release_version##*@}
if [[ -n ${RELEASE_TAG:-} && "$RELEASE_TAG" != "v$release_version" ]]; then
  echo "Tag $RELEASE_TAG must match Cargo version v$release_version" >&2
  exit 2
fi
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build -p nil --release --target "$release_target" --locked --offline
archive_name="nil-v$release_version-$release_target"
staging_dir=$(mktemp -d)
trap 'rm -rf "$staging_dir"' EXIT
mkdir -p "$staging_dir/$archive_name/examples" dist
cp "target/$release_target/release/nil" "$staging_dir/$archive_name/nil"
cp examples/add.nil "$staging_dir/$archive_name/examples/"
cp docs/RELEASES.md "$staging_dir/$archive_name/README.md"
chmod 755 "$staging_dir/$archive_name/nil"
tar -czf "dist/$archive_name.tar.gz" -C "$staging_dir" "$archive_name"
(cd dist && shasum -a 256 "$archive_name.tar.gz" > "$archive_name.tar.gz.sha256")
echo "dist/$archive_name.tar.gz"
