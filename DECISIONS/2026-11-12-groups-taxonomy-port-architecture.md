# Groups and Taxonomy Storage Sovereignty and Group Taxonomy Port

- Date: 2026-11-12
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-groups, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md), [2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md), [2026-11-12-forum-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-12-forum-taxonomy-sovereignty-and-port-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Social groups in `rustok-groups` organize communities by interests, topics, and categories.
The database schema (`m20260721_000001_create_groups.rs`) already reserves `category_id: Option<Uuid>` on the `groups` table, and `CreateGroupInput` accepts `category_id`.
However:
1. `category_id` was unvalidated: passing a non-existent or foreign category ID did not return a validation error.
2. `GroupSummary` and `GroupSummaryGql` did not project `category_id` or resolved category metadata (name, slug, icon, color).
3. `ListGroupsRequest` did not allow filtering groups by `category_id`.
4. Integrating taxonomy directly with foreign table queries or concrete database entities would violate Fluid Backend Architecture (FBA) and module storage sovereignty (`AGENTS.md` §5).

## Decision

Apply the Ports & Adapters (Hexagonal / FBA) architecture pattern to integrate `rustok-groups` with taxonomy:

### 1. Consumer-Owned Port (`GroupTaxonomyPort`)
- Define `GroupTaxonomyPort` in `crates/modules/rustok-groups/src/ports/taxonomy.rs`.
- The port specifies all taxonomy operations required during the group lifecycle:
  - `validate_category_exists`: Verifies category existence within the tenant and module scope ("groups") within an active transaction or connection.
  - `load_categories`: Loads localized category snapshots (`GroupTaxonomyCategorySnapshot`) by ID batch for group summaries, listings, and details.
  - `resolve_term_names`: Resolves tag term names for group tags.
  - `ensure_group_tags_in_tx`: Ensures tag terms exist in taxonomy during group mutations within the caller transaction.
- Define `GroupTaxonomyConnection<'a>` supporting `&DatabaseConnection`, `&DatabaseTransaction`, or `None`.

### 2. Standalone Mode & Graceful Degradation (`NoopGroupTaxonomyPort`)
- Provide `NoopGroupTaxonomyPort` in `rustok-groups::ports::taxonomy`:
  - `validate_category_exists` returns `Ok(true)` in standalone mode.
  - `load_categories` returns synthesized snapshots based on UUIDs.
  - `resolve_term_names` maps `term_id -> term_id.to_string()`.
  - `ensure_group_tags_in_tx` generates deterministic UUIDv5 identifiers.
- `GroupsService::new(db)` defaults to `NoopGroupTaxonomyPort`, allowing `rustok-groups` to run in isolated microservice environments and unit tests without taxonomy tables.
- `GroupsService::with_taxonomy_port` allows host/runtime injection of concrete taxonomy adapters.

### 3. Concrete Provider Adapter in `rustok-taxonomy`
- Implement `TaxonomyGroupTaxonomyPortAdapter` in `crates/modules/rustok-taxonomy/src/groups_taxonomy_port.rs`.
- `rustok-taxonomy` depends on `rustok-groups` (strictly acyclic: `taxonomy -> groups`).
- `rustok-groups` does NOT depend on `rustok-taxonomy` in production dependencies.

### 4. DTO & Query Capabilities
- Add `category_id: Option<Uuid>` to `GroupSummary` and `GroupSummaryGql`.
- Add `category: Option<GroupCategorySummary>` to `GroupSummary` when category details are resolved.
- Add `category_id: Option<Uuid>` to `ListGroupsRequest` and GraphQL query `groups(categoryId: ...)`.

## Invariants

- `rustok-groups` contains zero direct SQL queries against `taxonomy_*` tables and zero foreign keys to taxonomy tables.
- `rustok-groups/Cargo.toml` contains no production dependency on `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `taxonomy -> groups`.
- Standalone group operations succeed deterministically when taxonomy is disabled or absent.
