# rustok-docs / CRATE_API

## Public surface

### Public modules
- `controllers`
- `domain`
- `dto`
- `entities`
- `error`
- `migrations`
- `module`
- `search_projection`
- `services`

## Platform Capability Bindings
- `DocsModule` implements `rustok_core::module::RusToKModule` and `MigrationSource`.
- Declared as `ModuleKind::Optional`.
- Self-check returns `HealthStatus::Healthy`.
- Slug: `docs`.
- Dependencies: `outbox`, `flex`.
- Permissions: `PAGES_CREATE`, `PAGES_READ`, `PAGES_UPDATE`, `PAGES_DELETE`, `PAGES_LIST`, `PAGES_MANAGE`.
- Search Projection: registers `DocsSearchProjectionSourceFactory` for full-text search indexing.

## Domain Collaborations
- Sovereign module owner for collaborative documents, wiki pages, hierarchical page trees, and real-time knowledge bases powered by Loro CRDT.
- Universal Entity Reference (UER): embeds entities across modules (`commerce`, `blog`, `media`, `flex`) with backlinks recorded in `docs_entity_links`.
- Offline-first delta synchronization and multiplayer WebSockets (`DocsRoomHub`).
- Entities are crate-private; consumers use owner DTOs, services, and ports.

## Minimum Contract Set

### Input DTOs/Commands
- `CreatePageDto`: title, parent_id, slug, metadata JSON.
- `UpdatePageDto`: title, slug, metadata JSON, parent_id.
- `PageSummaryDto`: id, slug, title, parent_id, created_at, updated_at.
- `PageTreeNodeDto`: recursive tree structure for page hierarchies.
- `DocumentSyncDto`: client version vector and frontiers for differential syncing.
- `DocUpdateItemDto`: individual Loro delta update binary data.
- `ApplyDeltaDto`: sequence of updates to apply to a document.

### Domain Invariants
- Multi-tenant isolation: every read, write, snapshot, delta, and link enforces tenant authority. Slugs are unique per tenant (`idx_docs_pages_tenant_slug`).
- Hierarchical page tree: cycle detection prevents a page from becoming its own ancestor.
- Delta monotonicity: delta updates have monotonically increasing sequence numbers per document.
- Snapshot compaction: when uncompacted delta count reaches `COMPACTION_THRESHOLD` (50), deltas are folded into a new snapshot and older deltas are pruned.
- Offline-first conflict resolution: Loro CRDT guarantees eventual consistency without merge conflicts.

### Events / Outbox Side Effects
- Emits outbox events upon page creation, update, deletion, and hierarchy movement.
- Bi-directional entity backlinks maintained in `docs_entity_links`.

### Errors / Failure Codes
- `DocsError::PageNotFound`: document page does not exist in tenant.
- `DocsError::Validation`: invalid title, malformed slug, or schema violation.
- `DocsError::CycleDetected`: circular dependency in page parent hierarchy.
- `DocsError::Crdt`: corruption or error during Loro CRDT decoding or delta merge.
- `DocsError::Storage`: underlying database or persistence failure.
