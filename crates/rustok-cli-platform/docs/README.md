# `rustok-cli-platform` Documentation

`rustok-cli-platform` contains platform-level CLI command providers. These are
commands that belong to the platform toolchain itself rather than to a domain
module.

## Current State

- Provides the `core version` command.
- Exposes `command_provider()` for generated registry wiring.
- Depends on `rustok-cli-core`.
- Does not depend on `apps/server`, `rustok-cli` or domain crates.

## Target Direction

- Keep platform commands small and explicit.
- Move domain/module maintenance commands into module-local `cli/` adapter
  packages instead of adding them here.
- Keep `rustok-cli` focused on terminal dispatch and output.

