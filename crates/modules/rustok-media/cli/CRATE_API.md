# rustok-media-cli / CRATE_API

## Platform Capability Bindings
- Implements `CommandProvider` from `rustok-cli-core` for Media module CLI operational commands.
- Integrates with `rustok-storage`, `rustok-runtime`, and `sea-orm`.

## Domain Collaborations
- Collaborates with `rustok-media` services for asset storage reconciliation, orphan cleanup, and checksum verification.
- Dispatched by platform operator tooling and CLI runners.

## Public surface
- `pub struct MediaCommandProvider`: Command provider implementing `CommandProvider` trait for `media` namespace (`reconcile`).
- `pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider>`: Registration factory.

## Minimum Contract Set

### Input DTOs/Commands
- `media reconcile`: Command reconciling media persistence with physical storage files (accepts `--limit`, `--tenant-id`).

### Domain Invariants
- Bounded batch operations enforced (`RECONCILIATION_MAX_LIMIT`).
- Preserves structured outcome logs and evidence.
- Storage configuration strictly bound to local or configured cloud instance paths.

### Events / Outbox Side Effects
- Media reconciliation operations execute within safe read-only or marked cleanup sweeps without uncontrolled side effects.

### Errors / Failure Codes
- Configuration, storage, and database errors map to typed `CliCoreError` outcomes.
