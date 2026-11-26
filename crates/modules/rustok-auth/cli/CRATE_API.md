# rustok-auth-cli / CRATE_API

## Public surface

### Command Providers & CLI Entrypoints
- `AuthCommandProvider`: implements `CommandProvider` from `rustok-cli-core` to dispatch authentication and OAuth management commands.

### Commands
- `oauth create-app`: registers a new OAuth application for an explicitly specified tenant.
- `auth sessions-cleanup`: removes session rows (and cascading refresh history) only more than 30 days past expiry, using the Auth owner's bounded host/database clock checks and conservative margin. It is session-only and rejects dry-run rather than deleting.
- `auth jwt-authority-init --confirm`: offline one-time import of the trusted application JWT ledger into the separately configured empty PostgreSQL authority. Never overwrites an existing target.
- `auth jwt-witness-init --confirm`: initializes the third-store witness once against the trusted offline authority, preserving its ledger.
- `auth jwt-witness-recover --confirm`: copies only the latest authenticated witness snapshot into the offline pinned authority. Never lowers its revision or resets the witness.
- `auth jwt-revoke --purpose <purpose> --kid <id> --confirm`: permanently revokes an enrolled JWT key in the authoritative database. Requires the stable host MAC root in `RUSTOK_JWT_REVOCATION_ROOT`, never a command argument. The scope is deployment-wide, not tenant-scoped.

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
