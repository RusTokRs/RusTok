# Flex — Implementation Plan

> Canonical module documentation: [`README.md`](./README.md)
> Central module index: [`docs/modules/_index.md`](/docs/modules/_index.md)

---

## Execution checkpoint

- Current phase: phase5_standalone_no_compile_verification_handoff
- Last checkpoint: Attached field-definition and standalone GraphQL query/mutation roots, runtime handle, permission/error/event mapping and DTO moved to `flex::graphql`; attached field-definition row-to-core mapping, view-source, command-to-adapter-input mapping, persisted JSON shape helpers, create guardrails, persisted type-name normalization, lifecycle event construction and cache invalidation event taxonomy moved to `flex::registry`; standalone REST request/response DTO, request-to-command mapping and view mapping moved to `flex::rest`; standalone fields_config parsing/schema building/serialization, localized field-key derivation, row-to-view source mapping, entry normalize/defaults/strip/validate, shared/localized split, read resolution and PATCH merge helpers moved to `flex::standalone`; aggregate roots `FlexQuery` / `FlexMutation` declared through `[provides.graphql]` and are part of generated host composition, server registers only `FlexGraphqlRuntime` with concrete `FlexStandaloneSeaOrmService`, `FieldDefRegistry`, DB handle and cache adapter, and source-level boundary guards prohibit return of `apps/server/src/graphql/flex`, server-owned Flex REST DTO/command mapping, server-owned attached field-definition row/view constructors/command mapping/persisted JSON/lifecycle/cache invalidation policy, server-local standalone validation service, server-owned standalone fields_config/key interpretation, server-owned standalone row-to-view constructors and server-owned standalone entry split/merge helpers.
- Next step: Remove remaining Flex transport artifacts from server beyond Loco/Axum REST handler, SeaORM/bootstrap adapter layer; after compilations are allowed, run targeted Flex tests and record evidence.
- Open blockers: User explicitly requested no compilations for this iteration.
- Hand-off notes for next agent: No compilation was run by explicit request. Flex GraphQL is owner-owned and consumes shared `rustok_api::AuthContext` / `TenantContext`, `rustok_core::EventBus`, and host-provided `FlexGraphqlRuntime`. Flex REST DTO/command mapping ownership is now in `flex::rest`, attached field-definition row/view/command/persisted-JSON/lifecycle policy ownership is in `flex::registry`, and standalone fields_config/schema/key-derivation/row-view/entry normalization/validation/split/merge ownership is now in `flex::standalone`; server Flex responsibilities are now Loco/Axum REST handler extraction/routing, SeaORM persistence adapters, registry/cache/bootstrap wiring and schema composition data registration. Verify owner root composition and runtime injection with targeted Rust tests once compilation/test execution is allowed.
- Last updated at (UTC): 2026-07-02T00:00:00Z

## Scope of work

This plan locks the delivery of `flex` to the target capability-only state in three planes:

- attached-mode contracts and donor integrations;
- standalone schema/entry runtime and transport surfaces;
- manifest/module-system/governance contract without turning `flex` into owner donor persistence.

## Current state

`flex` already has a live attached-mode contract, live standalone GraphQL/REST surfaces in `apps/server` and a formalized Phase 4.6 module-system wiring as a capability-only ghost module.

## FFA/FBA status

- UI surfaces: none.
- FFA: `not_started` — module-owned UI for capability not declared.
- FBA: `in_progress` — attached field-definition and standalone GraphQL roots/runtime/DTO belong to `flex::graphql`, attached field-definition row/view/command mapping, persisted JSON shape, lifecycle policy helpers and cache invalidation event taxonomy belong to `flex::registry`, standalone REST DTO/command mapping belong to `flex::rest`, standalone fields_config/schema/key-derivation/row-view/entry validation/split/merge belongs to `flex::standalone`, roots are connected via manifest codegen and depend on host-composed `FlexGraphqlRuntime`; server remains the adapter/composition layer for Loco/Axum REST handler, SeaORM persistence, registry/cache/bootstrap wiring and DB/runtime injection.
- Structural shape: `no_ui_boundary`.

## Current status

> **Important note for following change sets:** old plans where multilingual copy lives inline in base rows or in canonical JSON blobs are no longer relevant.
> Current live contract for `flex`: `FieldDefinition.is_localized` already propagated through core/runtime/DB, registered attached consumers are now `user`/`product`/`order`/`topic`, standalone schema copy moved to `flex_schema_translations`, standalone entry values now split into `flex_entries.data` + `flex_entry_localized_values`, attached-mode generic localized-value storage moved to shared `crates/flex` and writes to `flex_attached_localized_values`, and standalone GraphQL and REST surfaces for schemas/entries are already live in `apps/server`. Cleanup migration removes residual inline locale-aware Flex payloads from donor metadata and standalone base rows. Rollout/governance contract for standalone is already locked via capability-only manifest, server-owned transport and repo-side verification; the next mandatory step is full integration verification and Phase 5 follow-up backlog.

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | Core types & validation in `rustok-core` | ✅ Done |
| Phase 1 | Migration helper, FlexError, FieldDefRegistry, events | ✅ Done |
| Phase 2 | Users (first consumer) | ✅ Done |
| Phase 3 | Admin API (GraphQL CRUD, RBAC, cache, pagination) | ✅ Done |
| Phase 4 | Attached-mode consumers (`user`, `product`, `order`, `topic`) | ✅ Closed: docs/migrator/is_localized aligned, generic localized-value storage exists, live donor read/write path closed for `user`, `product`, `order` and `topic` |
| Phase 4.5 | Extraction to `crates/flex` | 🔄 Nearly complete, verification/docs debts remain |
| Phase 4.6 | Ghost-module manifest integration | ✅ Done: `modules.toml` + `rustok-module.toml` + `FlexModule` + xtask/server/docs alignment |
| Phase 5 | Standalone mode | 🔄 In active implementation: schema-level copy, standalone entry-value split, GraphQL and REST surfaces already live; rollout/governance contract locked, integration verification and follow-up backlog not yet closed |
| Phase 6 | Advanced features | ⬜ Not started |

---

## Stages

Below, phases remain the canonical breakdown of implementation scope. Phase 4.x closes attached/runtime/module-system debts, Phase 5 covers the standalone surface, Phase 6 remains future backlog.

## Verification

- `cargo xtask validate-manifest`
- `cargo xtask module validate flex`
- `node scripts/verify/verify-flex-multilingual-contract.mjs`
- targeted `cargo check -p flex`

## Update rules

- change phase status only after code, docs and verification path are actually synchronized;
- do not return old assumptions about inline localized payload as canonical path to the plan;
- record staged rollout and external blockers explicitly, rather than hiding them under "almost ready" wording.

---

## Phase 4 — Attached mode debts

Flex in attached-mode can already store field definitions and route CRUD by
`entity_type`, but the current state is uneven:

- `user` — full schema CRUD path + donor write-path validation alive.
- `product` — schema CRUD registered in registry, donor write/read path now alive through shared attached localized storage.
- `order` / `topic` — schema CRUD registered in registry, donor write/read parity already done through shared attached localized storage,
  needs separate confirmation or explicit documentation as pending.
- `node` — appears in Flex module documentation as attached consumer, but in the current registry/API
  route for `node` is not mounted.

### Canonical scope / wiring

- [x] Lock the canonical list of live attached consumers
  - Live attached contract is currently limited to `user`, `product`, `order`, `topic`.
  - `node` is not considered a live attached consumer until a separate service/route and donor write-path appear.
- [x] Fix migrator ownership for attached migrations
  - `product_field_definitions` and `order_field_definitions` continue to come from owning crate migrations.
  - `topic_field_definitions` is connected in the canonical server `Migrator`.
- [x] Lock multilingual semantics field definitions
  - `FieldDefinition.is_localized` is now a mandatory part of the core/runtime/DB contract.
  - GraphQL, registry DTO and attached persistence must no longer treat localized/non-localized fields as an implicit agreement.

### Donor write-path parity

- [x] Confirm and lock donor write-path integration for `order`, `topic`
  - Override 2026-04-05: `topic` is no longer schema-only. Forum topics now use `forum_topics.metadata` plus `flex_attached_localized_values` under the same attached multilingual contract as `user`/`product`/`order`.
  - For `user`, validation/defaults/strip_unknown are already connected in the GraphQL mutation flow.
  - For `product`, live read/write path is already connected through shared attached localized storage in `crates/flex`.
  - For remaining attached consumers, either add a similar write-path, or explicitly mark current state as schema-only admin surface.
- [ ] Move localized attached values out of canonical JSON path
  - `is_localized = true` should not in the final state mean storing multilingual business value inside donor `metadata`.
  - Generic table `flex_attached_localized_values` is already introduced, and shared entity/helpers now live in `crates/flex`.
  - `user` and `product` already use this path in live read/write flow.
  - Cleanup/backfill of legacy inline payloads moved to separate migrations, runtime must no longer read donor/base-row inline localized copy as canonical fallback.

### Tests (integration pending)

- [x] Flex GraphQL CRUD: integration scenarios list/find/create/update/delete/reorder
  - `apps/server` now holds `schema.execute(...)` roundtrip for `createFieldDefinition` / `fieldDefinitions` / `fieldDefinition` / `updateFieldDefinition` / `reorderFieldDefinitions` / `deleteFieldDefinition` through live `FieldDefRegistry` routing.
- [x] Cache invalidation: integration/e2e scenarios on `FieldDefinition*` events
  - `field_definition_cache_from_context()` is covered by tests that run invalidation through live `EventBus` subscriber on `FieldDefinitionCreated`, `FieldDefinitionUpdated` and `FieldDefinitionDeleted`.
- [x] RBAC integration: explicit typed permission gates for Flex surfaces
  - Standalone GraphQL/REST surfaces use separate `flex_schemas:*` / `flex_entries:*` gates through `require_permission(...)` and `RequireFlex*` extractors.
  - Attached GraphQL read roots `fieldDefinitions` / `fieldDefinition` now also require explicit `flex_schemas:list/read` permissions, and targeted tests lock the denial path.
- [x] Attached validation flows: end-to-end verification of donor write-path where Flex is already declared live
  - `rustok-order` is now covered by targeted create-path scenarios: shared default values, localized attached split/persist and required-field rejection.
  - `rustok-forum` is now also covered by targeted topic create/read scenarios: shared defaults, localized attached split/persist, required-field rejection and read-side resolution from `flex_attached_localized_values`.
  - `rustok-commerce` is now covered by targeted product create/read/update scenarios: shared defaults, localized attached split/persist, required-field rejection and locale-fallback resolution from `flex_attached_localized_values`.

---

## Phase 4.5 — Extraction to `crates/flex`

Goal: move generic attached-mode contracts from `apps/server` to `crates/flex`,
leaving in `apps/server` only transport/adapters (GraphQL, RBAC gate, bootstrap wiring).

**Go/No-Go criteria for start:**
1. Attached-mode wiring debts for live consumers closed
2. Full integration run of Flex GraphQL CRUD + cache invalidation exists
3. No unclosed P1 bugs in current registry routing

### What is already moved

- [x] `crates/flex` created
- [x] Registry contracts (`FieldDefinitionService`, `FieldDefRegistry` adapter layer)
- [x] Generic CRUD orchestration helpers (registry lookup + CRUD/reorder routing)
- [x] `apps/server` uses direct imports from `flex` (without compatibility re-export)

### What remains to be moved

- [x] Cache invalidation hooks
  - Orchestration helpers `list_field_definitions_with_cache()` and `invalidate_field_definition_cache()` added in `crates/flex`
  - `apps/server` implements `FieldDefinitionCachePort` as adapter over the current cache implementation
- [x] Transport-agnostic error mapping
  - `map_flex_error()` moved to `crates/flex/src/errors.rs`
  - `apps/server` uses mapping from agnostic module
- [x] Migrate `user/product/order/topic` services to the new crate API
  - Bootstrap/GraphQL use direct `flex` API without changing GraphQL contracts
- [x] Removed legacy duplicate `crates/rustok-flex`
  - A single agnostic module `crates/flex` remains in the workspace
- [x] Remove duplication between `apps/server` and `crates/flex`
- [x] Write migration guide: `apps/server/docs/` + cross-link in `docs/index.md`

### What remains to be closed before phase finalization

- [ ] Full integration run of GraphQL CRUD + cache invalidation
  - Repo-side contract verification passes: `node scripts/verify/verify-flex-multilingual-contract.mjs` = `OK`.
  - Targeted Flex GraphQL tests should verify owner-owned `flex::graphql` roots through host-provided runtime without returning resolver/DTO to `apps/server/src/graphql/flex`; the previous server-local SQLite harness is no longer the target ownership point.
  - Duplicate registration for `m20260316_000004_create_topic_field_definitions` removed from server migrator; canonical migration continues to come from `rustok_forum::migrations()`.
  - 2026-06-13 no-compile iteration: product-side metadata update path patched in `crates/rustok-product/src/services/catalog.rs` so existing reserved product metadata survives Flex custom-field PATCH-style updates; targeted helper tests were added, but not executed by request.
- [x] Synchronize docs with actual registry routing and migrator ownership
  - `crates/flex/docs/README.md` aligned with live attached consumers (`user`, `product`, `order`, `topic`) without legacy `node`.
  - GraphQL contract and RBAC section in README now reflect actual `pagination`, `DeleteFieldDefinitionPayload` and typed `flex_schemas:*` / `flex_entries:*` gates.
- [x] Extract remaining server-side duplication to `crates/flex` only if it is truly a transport-agnostic contract, not an adapter concern
  - Duplicated `fields_config` parser for standalone GraphQL/REST moved to `crates/flex::parse_field_definitions_config()`.
  - Standalone GraphQL-specific `publish_event`, error mapping, response DTO mapping and RBAC checks moved to `crates/flex`; server-specific REST extractors and attached field-definition adapters remain in `apps/server`.

---

## Phase 4.6 — Ghost-module manifest integration

Goal: formalize `flex` as a capability / ghost module in the manifest-driven module system,
rather than as a "regular" domain module.

### Checklist

- [x] Add `crates/flex/rustok-module.toml`
  - Manifest aligned with capability modules like `alloy`, but without claim to donor persistence ownership.
- [x] Lock ghost module semantics in manifest and docs
  - `flex` extends donor modules custom contracts.
  - Attached-mode data remains in donor tables and donor write-path.
  - `FlexModule` publishes capability/runtime metadata and RBAC surface; attached field-definition and standalone GraphQL are owner-owned, REST DTO contract is also owner-owned in `flex::rest`, and server REST remains the handler adapter layer.
- [x] Define policy for runtime surfaces
  - Attached field-definition and standalone GraphQL owner-owned in `crates/flex` and declared through `[provides.graphql]`; REST request/response DTO owner-owned in `flex::rest`, live server adapter only mounts Loco/Axum routes, and the manifest does not attribute capability ownership to donor persistence.
  - Capability-only server feature `mod-flex` needed for registry/codegen wiring; the crate itself can remain an always-linked support dependency of the server.
- [~] Run manifest validation flow
  - `cargo xtask validate-manifest` / `cargo xtask module validate flex` are part of the acceptance path for `flex` and pass on the current workspace state
  - `cargo xtask module test flex` remains dependent on the general server test graph
- [x] Update central module docs after manifest appears
  - `docs/modules/_index.md`
  - `docs/modules/registry.md`
  - `docs/modules/manifest.md`
  - `xtask/README.md`

---

## Phase 5 — Standalone mode

Arbitrary schemas and records without binding to existing entities.

### What is already started

- [x] Transport-agnostic standalone contracts added in `crates/flex/src/standalone.rs`
  - DTO for schemas/entries (`FlexSchemaView`, `FlexEntryView`)
  - Commands and trait contract `FlexStandaloneService` for future adapter implementations
  - Basic guardrail validators for create/update commands (`validate_create_schema_command`, `validate_update_schema_command`, `validate_create_entry_command`, `validate_update_entry_command`)
  - Orchestration helpers (`list/find/create/update/delete` for schemas and entries), so adapters do not duplicate routing/pre-validation

### Tables

```sql
CREATE TABLE flex_schemas (
    id            UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    slug          VARCHAR(64) NOT NULL,
    fields_config JSONB NOT NULL,
    settings      JSONB NOT NULL DEFAULT '{}',
    is_active     BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, slug)
);

CREATE TABLE flex_schema_translations (
    schema_id     UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    locale        VARCHAR(32) NOT NULL,
    name          VARCHAR(255) NOT NULL,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (schema_id, locale)
);

CREATE TABLE flex_entries (
    id          UUID PRIMARY KEY,
    tenant_id   UUID NOT NULL,
    schema_id   UUID NOT NULL REFERENCES flex_schemas(id) ON DELETE CASCADE,
    entity_type VARCHAR(64),    -- NULL = standalone
    entity_id   UUID,           -- NULL = standalone
    data        JSONB NOT NULL,
    status      VARCHAR(32) NOT NULL DEFAULT 'draft',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, schema_id, entity_type, entity_id)
        WHERE entity_type IS NOT NULL AND entity_id IS NOT NULL
);
CREATE INDEX idx_flex_entries_data   ON flex_entries USING GIN (data);
CREATE INDEX idx_flex_entries_entity ON flex_entries (entity_type, entity_id);

CREATE TABLE flex_entry_localized_values (
    entry_id     UUID NOT NULL REFERENCES flex_entries(id) ON DELETE CASCADE,
    locale       VARCHAR(32) NOT NULL,
    tenant_id    UUID NOT NULL,
    data         JSONB NOT NULL DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (entry_id, locale)
);
CREATE INDEX idx_flex_entry_localized_values_owner
    ON flex_entry_localized_values (tenant_id, entry_id);
```

### Checklist

- [~] Migrations for `flex_schemas`, `flex_entries`
  - Migration file added: `m20260317_000001_create_flex_standalone_tables`
  - Migration connected in canonical server migrator
  - Separate follow-up migration slice schema-level localized copy moved from `flex_schemas` to `flex_schema_translations`
  - Separate follow-up migration slice standalone localized entry payload moved from inline `flex_entries.data` to `flex_entry_localized_values`
- [x] SeaORM entities *(added `flex_schemas`, `flex_entries`, `flex_schema_translations` and `flex_entry_localized_values` in `apps/server/src/models/_entities` + re-export in `models/`)*
- [x] Validation service (uses `CustomFieldsSchema` from core) *(`parse_standalone_fields_config`, `build_standalone_custom_fields_schema`, `serialize_standalone_fields_config`, `standalone_localized_field_keys`, standalone row-to-view source helpers, `normalize_and_validate_standalone_entry`, standalone shared/localized split, read resolution and PATCH merge helpers live in `crates/flex/src/standalone.rs`; server-local `flex_standalone_validation_service.rs` removed, SeaORM adapter delegates to owner helpers)*
- [x] CRUD services *(added SeaORM adapter `FlexStandaloneSeaOrmService` in `apps/server/src/services/flex_standalone_service.rs`, implementing `flex::FlexStandaloneService` with tenant-scoped CRUD for schemas/entries)*
- [x] Multilingual storage contract for standalone mode
  - schema-level localized copy (`name`, `description`) is no longer considered base-row data
  - `flex_schema_translations` is already the live storage path for schema-level copy
  - entry payload now split to `flex_entries.data` (shared) and `flex_entry_localized_values` (locale-aware values)
  - read/write service path already merges parallel localized rows back into effective entry payload
  - cleanup/backfill moved to follow-up migrations; runtime reads shared payload plus parallel localized rows
- [x] Events: `FlexSchemaCreated/Updated/Deleted`, `FlexEntryCreated/Updated/Deleted` *(event contracts + schema registry added in `rustok-events`; `crates/flex` provides transport-agnostic envelope/orchestration helpers and owner GraphQL publishes envelopes through shared `EventBus`, REST adapter publishes them from server)*
- [x] REST API: `/api/v1/flex/schemas`, `/api/v1/flex/schemas/{schema_id}/entries` *(live in `apps/server` as Loco/Axum handler adapter, tenant-scoped and with separate `flex_schemas:*` / `flex_entries:*` permission gates; request/response DTO, command mapping and view mapping owner-owned in `flex::rest`)*
- [x] GraphQL: `FlexSchema`, `FlexEntry`, queries/mutations *(owner-owned in `crates/flex/src/graphql`, connected through manifest-generated host schema, tenant-scoped and use separate `flex_schemas:*` / `flex_entries:*` permission gates)*
- [x] RBAC permissions: `flex.schemas.*`, `flex.entries.*`
  - Typed permissions present in `rustok-core`
  - GraphQL standalone surface uses separate `flex_schemas:*` and `flex_entries:*` gates
- [x] Indexer handler: `index_flex_entries` + `FlexIndexer` event handler
  - `rustok-index` now owns migration slice `index_flex_entries` and module-owned `flex_indexer`, which listens to `FlexEntry*`, `FlexSchemaUpdated/Deleted` and `ReindexRequested { target_type = "flex" }`.
  - `IndexModule` publishes `flex_indexer` through `register_event_listeners(...)`, and the server dispatcher includes it in runtime wiring alongside `content_indexer` and `product_indexer`.
- [x] Cascade delete: when deleting an entity, delete attached flex entries
  - Shared helper `delete_attached_localized_values()` lives in `crates/flex` and is connected in live hard-delete paths for `user`, `product` and `topic`.
  - Helper allows capability-optional test graphs without the `flex_attached_localized_values` table mounted, so isolated donor tests do not fail on cleanup paths.
  - For `order`, a separate hard-delete surface is not implemented in the current live contract; cleanup will be needed immediately when such a delete-path appears.
- [x] Guardrail: max relation depth = 1 (no recursive populate)
  - `crates/flex::validate_create_entry_command()` now explicitly forbids `entity_type = "flex_entry"`, so standalone `FlexEntry -> FlexEntry` chains are cut at the adapter/service layer and work identically for GraphQL and REST.
- [x] Resolve publish policy for standalone surface through ghost-module manifest
  - Standalone REST handler remains server-owned adapter layer, but REST DTO contract, command mapping and view mapping live in `flex::rest`.
  - `flex` publishes capability/runtime metadata through `rustok-module.toml`, `modules.toml` and `FlexModule`, without taking ownership of the transport surface.
  - Acceptance path: `cargo xtask validate-manifest`, `cargo xtask module validate flex`, `node scripts/verify/verify-flex-multilingual-contract.mjs`.
- [ ] Tests: unit + integration
  - `apps/server` already holds targeted REST roundtrip for standalone schema/entry CRUD and invalid payload rejection.
  - `apps/server` now also holds standalone GraphQL roundtrip for schema/entry CRUD and explicit denial-path for `flex_entries:create`.
  - Flex GraphQL verification should go through owner-owned `flex::graphql` roots and host-composed runtime; a heavy workspace migrator is not needed for a narrow Flex path.
  - Repo-side multilingual drift gate passes: `node scripts/verify/verify-flex-multilingual-contract.mjs`.
  - 2026-06-14 no-compile iteration: standalone contract guardrail tests added for untrimmed schema slugs, field keys and `entity_type`; localized entry row loading now includes tenant filtering to keep the parallel storage lookup tenant-scoped.
  - 2026-06-15 no-compile iteration: standalone contract validators now reject non-object schema settings, non-object entry data, untrimmed statuses and schemas with more than 50 fields; localized entry upsert lookup also filters by tenant.
  - 2026-06-16 no-compile iteration: standalone contract validators now enforce persistence-bound limits before adapter writes: schema slug <= 64, schema name <= 255, entry `entity_type` <= 64, entry `status` <= 32, schema names must not carry surrounding whitespace, and status values must be normalized machine identifiers.
  - 2026-06-19 no-compile iteration: SeaORM standalone adapter now reuses contract validators even for direct service calls, and entry update payloads behave as PATCH merges over the current effective shared + localized values so omitted localized keys are preserved.
  - 2026-06-19 no-compile follow-up: standalone orchestration rejects nil `schema_id`/`entry_id` before service delegation, create-entry commands reject nil `schema_id`/attached `entity_id`, and targeted unit coverage was added without compiling by request.
  - 2026-06-20 no-compile iteration: nil UUID validation was promoted to a public standalone boundary helper and reused by orchestration and the SeaORM adapter so direct schema/entry service calls reject nil tenant/schema/entry IDs before database access; localized entry map loading also had a duplicate await removed.
  - 2026-06-20 no-compile follow-up: optional `actor_id` validation now uses the same standalone nil UUID guardrail in orchestration and direct SeaORM mutation calls, with targeted boundary tests added but not executed by request.
  - 2026-06-20 no-compile schema-definition follow-up: standalone schema create/update validation now checks field-definition shape before adapter writes: non-empty localized labels/descriptions, select option presence/uniqueness, unsupported pattern rules, inverted min/max ranges, and invalid default values are rejected through targeted contract tests.
  - 2026-06-20 no-compile schema-definition hardening follow-up: standalone schema validators now also reject invalid/empty regex patterns, options on non-select fields, malformed localized validation error messages and negative field positions; targeted contract tests were added without execution by request.
  - 2026-06-20 no-compile schema-definition locale/rule follow-up: standalone schema validators now require normalized locale keys for localized labels/descriptions/error messages/select labels, reject empty optional localized maps, enforce select option values as normalized machine identifiers, and reject unsupported/negative min-max rules before adapter writes; targeted contract tests were added without execution by request.
  - 2026-06-21 no-compile handoff cleanup: standalone schema create/update validation now rejects empty or untrimmed schema descriptions before adapter writes, with targeted contract tests added without execution by request; the execution checkpoint also makes the deferred formatting/compile/test gates explicit for the next allowed verification pass.
  - 2026-06-24 no-compile iteration: standalone orchestration duplicate `update_schema` delegation was removed, and a source-level no-compile verifier `scripts/verify/verify-flex-standalone-contract.mjs` now locks the Phase 5 guardrails for nil UUIDs, schema-definition shape, localized-map normalization, tenant-scoped localized entry lookup and PATCH-style entry merge.
  - 2026-06-26 no-compile iteration: standalone schema-definition validation now rejects duplicate `position` values so schema field ordering is deterministic before adapter writes; the source-level verifier was extended to lock the unique-position guardrail.
  - Full closure of the item still requires a stable `rustok-server` test run; the current increment prepared standalone PATCH/guardrail/schema-definition fixes and tests, but compile/test evidence is deferred because this iteration was performed without compilations.
- [x] Documentation
  - Contracts, data model and live GraphQL/REST surfaces described
  - Rollout / governance contract for standalone surface documented as completed

### Standalone mode events

```rust
DomainEvent::FlexSchemaCreated { tenant_id, schema_id, slug }
DomainEvent::FlexSchemaUpdated { tenant_id, schema_id, slug }
DomainEvent::FlexSchemaDeleted { tenant_id, schema_id }
DomainEvent::FlexEntryCreated { tenant_id, schema_id, entry_id, entity_type, entity_id }
DomainEvent::FlexEntryUpdated { tenant_id, schema_id, entry_id }
DomainEvent::FlexEntryDeleted { tenant_id, schema_id, entry_id }
```

### Read model (indexer)

```sql
CREATE TABLE index_flex_entries (
    id            UUID PRIMARY KEY,
    tenant_id     UUID NOT NULL,
    schema_slug   VARCHAR(64) NOT NULL,
    entity_type   VARCHAR(64),
    entity_id     UUID,
    data_preview  JSONB,
    search_vector TSVECTOR,
    indexed_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (tenant_id, id)
);
CREATE INDEX idx_index_flex_search ON index_flex_entries USING GIN (search_vector);
```

### Open questions

1. **Schema versioning:** is schema change history needed?
2. **Migration on schema change:** how to migrate data when fields change?
3. **Rich text fields:** support Markdown/HTML in text fields?
4. **Computed fields:** are computed-on-the-fly fields needed?

---

## Phase 6 — Advanced (future)

- [ ] Conditional fields (show field B if field A = X)
- [ ] Computed fields (calculated from other fields)
- [ ] Field groups (sections in UI)
- [ ] Import/export of schemas between tenants
- [ ] Full-text search over custom fields via rustok-index
- [ ] Schema versioning (change history of definitions)
- [ ] Data migration tool (retro-validation of existing metadata)

---

## Tracking

When changing the plan:
1. Update this file
2. Update links and status in [`docs/modules/_index.md`](/docs/modules/_index.md) or [`docs/modules/registry.md`](/docs/modules/registry.md), if module composition/status changes
3. Run `cargo test -p rustok-core` — field_schema tests must pass
> **Live status override (2026-04-05):** attached multilingual donor path is already actually closed for `user`, `product`, `order` and `topic` through shared `flex_attached_localized_values`.
> `topic` is no longer a schema-level consumer: forum topic donor payload now lives in `forum_topics.metadata`, and locale-aware Flex values are resolved using the same effective locale contract as the other live donors.
> If underlying sections of the old plan say that `order` has not yet been migrated or that `topic` already has a donor metadata path, consider that outdated.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.
