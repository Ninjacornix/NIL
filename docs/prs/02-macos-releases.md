<!-- Title: chore(ci): publish macOS compiler binaries
Head: feat/macos-release-binaries
Base: feat/expr-profile-default
Merge method: Squash and merge
-->

## Purpose

Users should be able to download the NIL compiler without installing Rust. Package standalone compiler executables for Apple Silicon and Intel, and publish archives and checksums when a matching release tag is pushed.

## Changes

- Add a macOS packaging script for `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- Add native-architecture release tests, archive checksums, and packaged-execution smoke checks.
- Publish assets only after both architecture jobs pass; require the tag to match the Cargo version.
- Document installation, local packaging, and release publishing; ignore generated archives.

## Validation

```text
Complete-stack checks at ab37194:
./scripts/ci.sh — passed formatting, Clippy, build, and 121 tests.
./scripts/package-macos.sh aarch64-apple-darwin — passed.
Packaged compiler: check examples/add.nil — passed; run — returned 42.
/tmp/nil-actionlint/actionlint .github/workflows/release.yml — passed.

Packaging was checked on Apple Silicon using the complete stack, which includes
later compiler changes. Intel execution and tag-triggered GitHub publication
were not run locally; the workflow defines those checks. No compiler IR or
diagnostic golden fixtures change in this PR.
```

## Issues and compatibility

None for language semantics or parser compatibility. Packages target macOS 11+; older systems are not tested. Releases are unsigned and not notarized. Compiler binaries and generated-program target support are separate concerns.

See [release requirements](https://github.com/Ninjacornix/NIL/blob/feat/macos-release-binaries/docs/RELEASES.md). No linked issue.
