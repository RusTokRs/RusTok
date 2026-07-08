# rustok-cli-core

## Purpose

`rustok-cli-core` owns stable contracts for the RusToK platform CLI and module command
providers.

## Responsibilities

- Describe command providers without tying module domain crates to `clap`, stdout, or exit
  handling.
- Provide command request/outcome contracts for the future `rustok-cli` binary.
- Keep production server runtime independent from maintenance and distribution tooling.

## Entry Points

- `CommandDescriptor`
- `CommandRequest`
- `CommandOutcome`
- `CommandProvider`
- `CliCoreError`

## Interactions

- Future module-local `cli/` adapter packages may depend on this crate.
- The user-facing binary can be named `rustok-cli`.
- `apps/server` must not depend on this crate for production HTTP runtime behavior.

See [docs](docs/README.md).

