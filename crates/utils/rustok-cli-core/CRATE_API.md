# rustok-cli-core / CRATE_API

## Role

`rustok-cli-core` defines the fundamental abstractions, traits, and data contracts for the RusToK platform CLI ecosystem.
It allows domain modules and platform subsystems to expose executable commands without depending on terminal UI libraries or argument parsers.

## Public surface

### Primary Public Types

- `CommandDescriptor`: Metadata describing an individual command (`namespace`, `name`, `summary`, `supports_dry_run`).
- `CommandRequest`: Execution payload containing namespace, name, parsed JSON arguments, and dry-run flag.
- `CommandOutcome`: Execution result containing exit code, output message, and optional JSON data payload.
- `CliCoreError`: Typed error variants (`UnknownCommand`, `InvalidInput`, `CommandFailed`, `DryRunNotSupported`).
- `CliCoreResult`: Type alias for results returning `CliCoreError`.

### Primary Public Traits

- `CommandProvider`: Trait implemented by CLI extension providers (`commands()`, `execute()`).

## Invariants

- **Terminal Agnostic**: Core contracts depend solely on serializable data and asynchronous execution, with zero dependencies on terminal formatting or shell environments.
- **Dry-Run Enforcement**: Command providers declare dry-run support explicitly via `supports_dry_run`; commands rejecting dry-run must return `CliCoreError::DryRunNotSupported`.
- **Safe Fallback**: Default trait implementation for `CommandProvider::execute` fails closed with `CliCoreError::UnknownCommand`.

