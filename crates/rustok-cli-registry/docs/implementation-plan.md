# Implementation plan for `rustok-cli-registry`

## Status

`in_progress`

## Current State

- Selected distribution registry crate exists outside the runner and server.
- `selected_distribution_registry()` loads providers from generated source.
- Generated source is produced by `scripts/generate/generate-cli-registry.mjs`.
- Root `cli-registry.toml` selects the platform provider for `core version`.
- Generator freshness checks also verify selected provider dependencies in
  `rustok-cli-registry/Cargo.toml`.

## Target State

- The registry is generated from `modules.toml` and module-local `rustok-module.toml` metadata.
- Module-local `cli/` adapter crates are connected through `[provides.cli]`.
- The registry depends only on selected command provider crates and `rustok-cli-core`; it does not depend on the runner, server runtime or domain crates without an adapter boundary.
- Production HTTP builds remain independent from CLI provider aggregation.

## Next Steps

1. Add the first module-local or platform ops provider metadata through `[provides.cli]`.
2. Add generator validation for selected provider crate dependencies once real providers exist.
3. Use the generated registry when migrating the first legacy task, seed or migration command.

## Verification

- `node scripts/generate/generate-cli-registry.mjs --check`
- `cargo test -p rustok-cli-registry --quiet`
- `node scripts/verify/verify-api-surface-contract.mjs`
