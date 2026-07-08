# `rustok-cli` Documentation

`rustok-cli` is the thin user-facing runner for platform and module maintenance
commands. It starts with built-in discovery/help behavior and a typed provider
dispatch seam; real maintenance commands must arrive through explicit provider
registries, not through hardcoded server-owned task calls.

## Current State

- Provides the `rustok-cli` binary.
- Uses `rustok-cli-core` contracts for command descriptors.
- Uses `rustok-cli-registry` for selected distribution providers.
- Does not manually enumerate module command providers.
- Exposes `rustok-cli list` for the current distribution command inventory.
- Exposes `rustok-cli list --json` for automation and generated distribution
  registry checks.
- Supports namespace-scoped discovery through `rustok-cli list --namespace <name>`.
- Dispatches `rustok-cli <namespace> <command>` through `CommandProvider::execute`.
- Normalizes provider input into `CommandRequest.args.options` and
  `CommandRequest.args.positionals`; option names use `snake_case`.
- Provides `rustok-cli core version` as the first built-in typed execution command.
- Owns an explicit `CommandRegistry` that aggregates providers and rejects duplicate
  `namespace/name` command registrations.
- Does not depend on `apps/server` or `loco-rs`.

## Target Direction

- Populate the generated distribution registry with selected module-local
  command providers.
- Move legacy Loco task, seed and migration flows into typed CLI commands.
- Keep module command adapters next to the owning module, outside domain core.
- Keep the production server binary focused on HTTP runtime startup.
