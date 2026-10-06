# Semantic provider bundles

`sequence/` implements the migrated equality algorithm as a portable typed HIR
recipe. Its equivalent NIL source is explanatory; the HIR recipe is authoritative.
`example/` is an independently loaded record-update provider and client.

See [authoring and loading](../docs/architecture/PLUGINS.md) and
[ADR 029](../docs/adr/029.md). All files use the repository MIT licence.
Version 1 accepts validated, nonallocating/no-host-effect local NIL providers;
it is not a native shared-library loader or a stable C ABI, and makes no sandbox
claim. No registry, package manager or remote loading is supplied.
