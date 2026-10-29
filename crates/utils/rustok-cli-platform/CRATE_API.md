# rustok-cli-platform / CRATE_API

## Role

`rustok-cli-platform` provides platform-level CLI command implementations for core RusToK maintenance and operational procedures.
It executes database query baselines, queued manifest-derived build plans, and schema migrations outside of the HTTP server.

## Public surface

### Primary Public Types

- `PlatformCommandProvider`: Implements `CommandProvider` from `rustok-cli-core` for platform-level commands (`core version`, `core db-baseline`, `core rebuild`, `migrate up`, `migrate status`).

### Primary Functions

- `command_provider`: Constructs a boxed `dyn CommandProvider` initialized with the host runtime composition.

## Invariants

- **Server Decoupling**: Platform operations execute directly against the database or runtime composition without importing `apps/server` or launching web servers.
- **Dry-Run Support**: Destructive or generative commands (such as `core rebuild`) declare dry-run support and simulate outputs when requested.
- **Typed Outcomes**: All commands return standardized `CommandOutcome` structures with structured machine-readable JSON data payloads.

