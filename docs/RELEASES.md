# macOS compiler releases

NIL ships a standalone `nil` executable for Apple Silicon
(`aarch64-apple-darwin`) and Intel (`x86_64-apple-darwin`). Rust is needed to build
it, but is not needed to run a downloaded release. The executable parses, checks,
and interprets NIL; generating native binaries from NIL programs is a later milestone.

## Install and run

Download the matching `.tar.gz` and `.sha256` files from GitHub Releases.
Verify and extract them, substituting your version and architecture:

```sh
shasum -a 256 -c nil-v0.1.0-aarch64-apple-darwin.tar.gz.sha256
tar -xzf nil-v0.1.0-aarch64-apple-darwin.tar.gz
cd nil-v0.1.0-aarch64-apple-darwin
./nil --version
./nil check examples/add.nil
./nil run examples/add.nil
# 42
```

Move `nil` into a directory on your PATH if desired. Binaries target macOS 11+
using Apple's system libraries; CI executes both architectures on macOS 15.
Older macOS versions are not currently tested. Releases are unsigned and not
notarized, so macOS may require approval for a downloaded executable.

## Build locally

Install the Rust toolchain pinned in `rust-toolchain.toml` and Apple's command-line
developer tools. Install the target, then package it:

```sh
rustup target add aarch64-apple-darwin
./scripts/package-macos.sh aarch64-apple-darwin
# Intel: rustup target add x86_64-apple-darwin
# ./scripts/package-macos.sh x86_64-apple-darwin
```

Archives and SHA-256 checksums appear in `dist/`. Cross-compiling on macOS is
supported; executing the resulting binary requires the matching architecture
or an available compatibility layer.

## Publish a release

1. Update `[workspace.package].version` in `Cargo.toml` and regenerate `Cargo.lock`.
2. Land and validate the release commit, including `.github/workflows/release.yml`.
3. Create and push a matching tag, for example `git tag v0.1.0` followed by
   `git push origin v0.1.0`.

The workflow tests both native architectures, verifies packaged execution and
checksums, then creates a GitHub release with four assets: two archives and their
checksums. Tags must match the Cargo version. Tags containing `-` produce prereleases.
Manual workflow runs and pull requests build and test without publishing.
Publishing requires both builds to pass. Existing releases are never overwritten;
to retry publishing after a release already exists, manage its assets explicitly.
Signing, notarization, Linux, Windows and native NIL code generation are outside
this change.
