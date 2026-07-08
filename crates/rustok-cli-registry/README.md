# rustok-cli-registry

## Purpose

`rustok-cli-registry` owns the selected distribution command provider registry
for the RusToK platform CLI.

## Responsibilities

- Expose the command providers selected for the current platform distribution.
- Keep provider aggregation outside the user-facing CLI runner.
- Stay independent from `apps/server`, domain crates and terminal parsing.
- Provide the generated registry entrypoint for module-local `cli/` adapters.

## Entry Points

- `SelectedDistributionRegistry`
- `selected_distribution_registry`
- Generated source: `src/generated.rs`

## Interactions

- Depends on `rustok-cli-core` only.
- Depends on selected provider crates such as `rustok-cli-platform`.
- Consumed by `rustok-cli`.
- Generated distribution code updates `src/generated.rs` from `[provides.cli]`
  metadata and root `cli-registry.toml` when command providers are selected.

See [docs](docs/README.md).
