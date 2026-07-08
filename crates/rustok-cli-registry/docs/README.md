# `rustok-cli-registry` Documentation

`rustok-cli-registry` is the selected-distribution registry for RusToK CLI command
providers. It is intentionally separate from both the CLI runner and module
domain crates.

## Current State

- Provides a selected distribution registry backed by generated source.
- Exposes provider references for the `rustok-cli` runner.
- Depends on `rustok-cli-core` and selected provider crates.
- Does not depend on `apps/server`, `rustok-cli`, domain crates or terminal
  parsing libraries.
- Uses `scripts/generate/generate-cli-registry.mjs --check` to keep generated
  source in sync with root `cli-registry.toml` and module manifests.
- The generator also verifies that selected provider crates are declared as
  workspace dependencies of `rustok-cli-registry`.

## Target Direction

- Generate this registry from distribution/module manifests.
- Connect module-local `cli/` adapter packages through explicit provider entries.
- Keep real command implementation beside the owning module or integration.
- Keep the production HTTP server free from CLI provider aggregation.
