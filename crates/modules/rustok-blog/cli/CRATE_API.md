# rustok-blog-cli / CRATE_API

## Platform Capability Bindings
- Implements `CommandProvider` from `rustok-cli-core` for Blog module CLI operational commands.
- Integrates with `rustok-blog`, `rustok-outbox`, `rustok-runtime`, and `sea-orm`.

## Domain Collaborations
- Collaborates with `rustok-blog` services for scheduled post publication and preview token cleanup.
- Dispatched by platform operator tooling and CLI runners.

## Public surface
- `pub struct BlogCommandProvider`: Command provider implementing `CommandProvider` trait for `blog` namespace (`publish-scheduled`, `cleanup-tokens`).
- `pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider>`: Registration factory.

## Minimum Contract Set

### Input DTOs/Commands
- `blog publish-scheduled`: Command publishing due scheduled blog posts (accepts `--limit`, `--tenant-id`, `--dry-run`).
- `blog cleanup-tokens`: Command removing expired preview tokens (accepts `--limit`, `--tenant-id`, `--dry-run`).

### Domain Invariants
- Bounded batch operations enforced (`DEFAULT_LIMIT`, `MAX_LIMIT`).
- Preserves structured outcome logs and machine-readable data.
- Tenant isolation respected: commands operate either per explicitly specified tenant or iteratively across distinct tenants with pending work.
- Dry-run mode evaluates and reports pending actions without executing mutations or emitting outbox events.

### Events / Outbox Side Effects
- When not in `--dry-run`, publishing scheduled posts emits canonical `PostPublished` outbox events through `TransactionalEventBus`.
- Preview token cleanup deletes expired token rows without side effects on post content.

### Errors / Failure Codes
- Validation, database, and transaction errors map to typed `CliCoreError` outcomes.

