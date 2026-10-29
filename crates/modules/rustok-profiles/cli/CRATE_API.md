# rustok-profiles-cli / CRATE_API

## Public surface

### Command Providers & CLI Entrypoints
- `ProfilesCommandProvider`: implements `CommandProvider` from `rustok-cli-core` to dispatch profile operational commands.

### Commands
- `profiles backfill`: backfill missing user profile records for tenant users with dry-run support.

## Platform Capability Bindings
- **CLI Runtime Dispatch**: Integrates into the host operator CLI runtime through `CommandProvider`.
- **Tenant & Identity Composition**: Composes user identity resolution across Auth, Customer, and Tenant domain services.

## Domain Collaborations
- **`rustok-profiles`**: Core profile domain module providing `ProfileService`, `ProfileMutationService`, and `ProfilesReader`.
- **`rustok-auth`**: Reads authentication user records via `AuthUserBackfillReadPort`.
- **`rustok-customer`**: Reads customer profile enrichment data.
- **`rustok-tenant`**: Validates tenant scope and resolves tenant boundaries via `TenantReadPort`.
- **`rustok-outbox`**: Dispatches profile backfill side effects via transactional outbox.

## Minimum Contract Set

### Input DTOs/Commands
- `CommandRequest`: structured command invocation parameters including tenant, arguments, and dry-run flags.
- `CommandDescriptor`: metadata describing command names, usage, and capability flags.
- `ProfileBackfillRequest`: backfill parameter specification for profile batch synchronization.

### Domain Invariants
- **Dry-Run Safety**: Backfill commands supporting dry-run must compute operations without committing database mutations.
- **Tenant Isolation**: Operations are strictly scoped to the specified tenant or execute cross-tenant backfills explicitly with tenant authorization.
- **Fail-Closed Verification**: Partial read failures during user or tenant lookup must return typed domain errors rather than skipping records silently.

### Events / Outbox Side Effects
- Emits profile creation and update events through `TransactionalEventBus` when dry-run is disabled.

### Errors / Failure Codes
- `CliCoreError`: standardized CLI execution errors (e.g. invalid arguments, execution failure).
- Failure codes: `profiles.backfill_host_unavailable`, `profiles.backfill_tenant_read_failed`, `profiles.backfill_user_read_failed`, `profiles.backfill_enrichment_read_failed`.
