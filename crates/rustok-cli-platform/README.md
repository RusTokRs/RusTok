# rustok-cli-platform

## Purpose

`rustok-cli-platform` owns platform-level CLI command providers that are not part
of a domain module and do not belong in the terminal runner.

## Responsibilities

- Provide platform/core command providers for `rustok-cli-registry`.
- Keep platform command execution outside `rustok-cli`.
- Depend only on CLI contracts and narrowly required data-format helpers.
- Stay independent from `apps/server` and domain modules.

## Entry Points

- `PlatformCommandProvider`
- `command_provider`

## Interactions

- Depends on `rustok-cli-core`.
- Is selected through `cli-registry.toml` and generated into
  `rustok-cli-registry`.
- Is consumed by `rustok-cli` only through the selected registry.

See [docs](docs/README.md).

