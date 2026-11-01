# rustok-installer-cli — Crate API

Selected `rustok-cli` provider for installer operator commands.

## Public surface

- `InstallerCommandProvider` — the command provider implementation registered with the platform CLI.
- `command_provider(runtime)` — constructs the provider from the runtime composition.

## Invariants

- The crate owns terminal-adapter mapping only; plan, preflight, and seed semantics are delegated to `rustok-installer`.
- No installer state is owned here: the provider translates CLI requests into typed installer contracts and maps outcomes back to terminal output.
