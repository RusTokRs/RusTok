# rustok-content-revisions / CRATE_API

## Public surface
- `ContentRevisionService`: unified cross-module revision tracking and retrieval service.
- `ContentRevisionApi`: trait abstraction for content revision operations.
- `ContentRevisionConfig`, `ContentTypeConfig`: content type registration and retention policy configuration.
- `Migrator`: SeaORM migration descriptor for content revision schema setup.
- `ContentRevisionError`: strongly-typed revision integration errors.
- Re-exports from `rustok-revisions`: `ChangeSource`, `RetentionPolicy`, `Revision`, `RevisionDiff`, `RevisionEvent`, `RevisionMetadata`, `RevisionTracker`, `Revisionable`.

## Platform Capability Bindings
- **Cross-Module Content Revisions**: Provides unified revision tracking across content modules (blog, forum, pages).
- **Audit & Historical Replay**: Maintains structured JSON patches and snapshot diffs.

## Domain Collaborations
- Integrates with `rustok-revisions` engine as persistence and diffing driver.
- Coordinates with content-bearing modules to record author provenance and change metadata.

## Minimum Contract Set

### Input DTOs/Commands
- `ContentRevisionService::track_create`: registers initial version of a content entity.
- `ContentRevisionService::track_update`: computes diff and persists delta revision record.
- `ContentRevisionService::track_delete`: marks tombstone or terminal revision state.

### Domain Invariants
- Entity and tenant identifiers must be valid, non-nil UUIDs.
- Revision sequence numbers monotonically increase without gaps.
- Payloads must implement `serde::Serialize` and `serde::de::DeserializeOwned`.

### Events / Outbox Side Effects
- Dispatches revision change events through registered module event sinks.

### Errors / Failure Codes
- `ContentRevisionError::RevisionEngineError`: internal failure in `rustok-revisions` driver.
- `ContentRevisionError::SerializationError`: JSON serialization or deserialization failure.
- `ContentRevisionError::DatabaseError`: SeaORM database transaction or query error.

