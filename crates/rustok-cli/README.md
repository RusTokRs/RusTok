# rustok-cli

## Purpose

`rustok-cli` is the user-facing platform CLI runner for RusToK maintenance,
distribution and module command entrypoints.

## Responsibilities

- Own terminal-facing argument dispatch, help output and exit codes.
- Consume command/provider contracts from `rustok-cli-core`.
- Aggregate commands from `rustok-cli-registry`.
- Reject duplicate `namespace/name` command registrations before execution.
- Dispatch `rustok-cli <namespace> <command>` to selected providers.
- Normalize command arguments into `CommandRequest.args.options` and
  `CommandRequest.args.positionals`.
- Stay outside the production HTTP server runtime.

## Entry Points

- Binary: `rustok-cli`
- Library helpers: `CommandRegistry`, `run_with_args`, `collect_commands`,
  `render_command_list`, `render_command_list_json`

## Interactions

- Depends on `rustok-cli-core`.
- Depends on `rustok-cli-registry` for selected distribution providers.
- Future module-local `cli/` adapter crates can provide commands through generated registries.
- Exposes `rustok-cli list --json` as a stable machine-readable command inventory
  for future platform assembly tooling.
- Supports `rustok-cli list --namespace <name>` so discovery can stay scoped as
  module command providers grow.
- Supports namespace command execution through `CommandProvider::execute`.
- Provides `rustok-cli core version` as the first built-in typed provider command.
- Must not depend on `apps/server` or domain crates directly as a central command dump.

See [docs](docs/README.md).
