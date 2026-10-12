# rustok-docs / CRATE_API

## Role

`rustok-docs` is the sovereign module owner for collaborative documents, wiki pages,
hierarchical page trees, and real-time Notion-grade knowledge bases across RusToK.

Persistence entities are **not** public API. Consumers use owner DTOs, services,
transport roots, or explicit ports.

## Public Surface

Deliberate public modules and facades:

- `controllers` — Axum REST endpoints (`router`, `axum_router`, `DocsHttpRuntime`, page CRUD, tree hierarchy, markdown export, delta sync) and WebSocket session management (`ws_handler`, `DocsRoomHub`, `ClientMessage`, `ServerMessage`);
- `domain` — CRDT core (`CollaborativeDoc`), block models (`DocBlock`, `BlockType`), Universal Entity Reference (`EntityRef`, `EntityDisplayMode`);
- `dto` — Owner command/query/response contracts (`CreatePageDto`, `UpdatePageDto`, `PageSummaryDto`, `PageTreeNodeDto`, `DocumentSyncDto`, `DocUpdateItemDto`, `ApplyDeltaDto`);
- `error` — `DocsError`, `DocsResult`;
- `migrations` — Module migration source (`migrations()`, `migration_dependencies()`);
- `module` — `DocsModule` implementing `RusToKModule` and `MigrationSource`;
- `services` — Owner service (`DocumentService`, `COMPACTION_THRESHOLD`).

`entities` is crate-private. Direct cross-crate SeaORM access is strictly forbidden by repository platform rules.

Primary entry points:

- `DocsModule`
- `DocumentService`
- `CollaborativeDoc`
- `DocBlock`
- `EntityRef`

## Persistence & Compaction Contract

1. **Storage Topology:**
   - `docs_pages`: Metadata, tenant isolation, hierarchical `parent_id` tree, unique slug.
   - `docs_snapshots`: Periodic full Loro binary snapshots (`seq_id`, `snapshot_data`, `frontiers`).
   - `docs_updates`: Monotonically sequenced append-only binary delta updates (`(page_id, seq_id)` composite uniqueness).
   - `docs_entity_links`: Reverse backlinks ledger for all embedded UER (`EntityRef`) blocks.

2. **Snapshot Compaction Invariant:**
   - Delta updates are logged monotonically with atomic sequence numbers.
   - When the uncompacted delta count reaches `COMPACTION_THRESHOLD` (50), `DocumentService` compacts accumulated updates into a new `docs_snapshots` record and prunes old updates up to that sequence number.
   - At least the latest snapshot is always preserved.

3. **Multi-Tenant Authority:**
   - Every read, mutation, snapshot, and backlink operation enforces `tenant_id` authority.
   - Slugs are uniquely constrained per tenant (`idx_docs_pages_tenant_slug`).
   - Cross-tenant document lookups fail closed with `PageNotFound`.

## Universal Entity Reference (UER) Embedding Contract

Embedded entities are declared via `BlockType::EntityEmbed(EntityRef)`:
- `module`: Target module slug (e.g. `commerce`, `blog`, `media`, `flex`);
- `entity_type`: Target entity identifier (e.g. `product`, `post`, `asset`);
- `entity_id`: Entity UUID;
- `display_mode`: `Inline`, `Card`, `Embed`, or `DatabaseRow`.

Embedded entity backlinks are indexed into `docs_entity_links` to support bi-directional cross-module graph navigation.
