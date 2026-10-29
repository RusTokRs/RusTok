# rustok-auth-cli / CRATE_API

## Public surface

### Command Providers & CLI Entrypoints
- `AuthCommandProvider`: implements `CommandProvider` from `rustok-cli-core` to dispatch authentication and OAuth management commands.

### Commands
- `oauth create-app`: registers a new OAuth application for an explicitly specified tenant.
- `auth sessions-cleanup`: removes expired session and token records.

## Platform Capability Bindings
- **CLI Runtime Dispatch**: Integrates into the host operator CLI runtime through `CommandProvider`.
- **Tenant Scope Enforcement**: Requires explicit tenant selection for OAuth client registration.

## Domain Collaborations
- **`rustok-auth`**: Core authentication module providing password hashing and token generation.
- **`rustok-cli-core`**: CLI framework providing command descriptors, requests, and outcomes.
- **`rustok-runtime`**: Platform runtime providing database connection handles.

## Minimum Contract Set

### Input DTOs/Commands
- `CommandRequest`: structured command invocation parameters including namespace, name, and options.
- `CommandDescriptor`: metadata describing command names, usage, and capability flags.

### Domain Invariants
- **Non-Swallowed Errors**: Session cleanup and application registration failures return typed domain errors.
- **Explicit Tenant Scoping**: Creating an OAuth application requires an explicit, non-empty tenant identifier.
- **Safe Credential Generation**: Client secrets and tokens are cryptographically generated using standard auth utilities.

### Events / Outbox Side Effects
- Mutates OAuth application tables and purges expired session records in persistent storage.

### Errors / Failure Codes
- `CliCoreError`: standardized CLI execution errors (e.g. invalid arguments, execution failure, missing tenant).
