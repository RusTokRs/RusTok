# Implementation plan for `rustok-cli-platform`

## Status

`in_progress`

## Current State

- Platform provider crate exists outside the CLI runner.
- `core version` is implemented through `CommandProvider::execute`.
- The provider is selected through root `cli-registry.toml` and generated into
  `rustok-cli-registry`.

## Target State

- Platform CLI commands live here only when they are not owned by a domain module.
- Module-specific maintenance commands live in module-local `cli/` adapter
  packages.
- The production HTTP server does not depend on this crate.

## Next Steps

1. Add the first module-local provider through `[provides.cli]`.
2. Move the first server task or seed into a typed provider command.
3. Keep generated registry checks strict as selected providers grow.

## Verification

- `cargo test -p rustok-cli-platform --quiet`
- `node scripts/generate/generate-cli-registry.mjs --check`
- `node scripts/verify/verify-api-surface-contract.mjs`

