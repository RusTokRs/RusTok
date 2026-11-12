# Forum and Taxonomy storage sovereignty and Forum port decoupling

- Date: 2026-11-12
- Decision status: Proposed
- Implementation status: In progress
- Owners: rustok-forum, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md), [2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md](./2026-11-11-blog-taxonomy-sovereignty-and-port-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Following the decoupling of `rustok-product` via `ProductTaxonomyPort` and `rustok-blog` via `BlogTaxonomyPort`, an architectural review of `rustok-forum` revealed that Forum remains tightly coupled to `rustok-taxonomy`:

1. **Direct Service, Reader & Transaction Coupling**:
   - `CategoryService`, `CategoryTaxonomyReadService`, `CategoryTaxonomyTreeReadService`, `CategoryCommandService`, `CategoryLifecycleService`, `CategoryAudienceService`, `CategoryVisibilityService`, and `CategorySearchScopeService` directly invoke `TaxonomyOwnerCategoryReader`, `TaxonomyService`, `sync_module_category_with_owned_aliases_in_tx`, `lock_category_hierarchy_writer_in_tx`, `move_module_category_in_tx`, `reorder_module_category_siblings_in_tx`, and `shift_module_category_siblings_for_insert_in_tx`.
   - `ForumCategoryRouteService` invokes `TaxonomyService::resolve_term_route_for_module` and `TaxonomyOwnerCategoryReader::load_scoped_categories` directly to resolve category routes and canonical slugs.
   - `TopicService` calls `TaxonomyService::resolve_term_names` and `TaxonomyService::ensure_terms_for_module_in_tx` directly for topic tags.
   - `ForumSearchProjectionService` and `ForumReadModelOwnerService` call `TaxonomyOwnerCategoryReader` directly to assemble category read models and search documents.
   - `ForumError` contains `impl From<rustok_taxonomy::TaxonomyError> for ForumError`.
   - `crates/modules/rustok-forum/Cargo.toml` declares a mandatory dependency on `rustok-taxonomy`.

2. **Migration Coupling**:
   - Migrations `m20260823_000030_backfill_forum_categories_to_taxonomy.rs` and `m20260824_000031_retire_forum_category_legacy_storage.rs` import living `rustok_taxonomy` entities and normalization helpers directly.

3. **Impact on Fluid Backend Architecture (FBA) & Microservice Sovereignty**:
   - A standalone forum service or isolated test harness cannot start or execute read/write operations against an isolated database schema containing only forum tables (`forum_topics`, `forum_replies`, `forum_categories`, `forum_topic_tags`, etc.).
   - Any query or mutation triggers missing-table or missing-service errors when `taxonomy_terms` is absent.
   - This violates `AGENTS.md` §5 (module sovereignty, zero cross-module private table reads, no shadow/coupled dependencies).

## Decision

Apply the Ports & Adapters (Hexagonal / FBA) architecture pattern to decouple `rustok-forum` from `rustok-taxonomy`:

### 1. Consumer-Owned Port (`ForumTaxonomyPort`)
- Define `ForumTaxonomyPort` in `rustok-forum::ports::taxonomy`.
- The port specifies all taxonomy operations required during the forum category and topic tag lifecycle:
  - **Category Hierarchy & Locks**:
    - `lock_category_hierarchy_in_tx`: lock category hierarchy writer inside transaction.
    - `load_category_parents`: load parent mapping for categories.
    - `load_category_siblings_in_tx`: load sibling category IDs under a parent in transaction.
    - `shift_category_siblings_for_insert_in_tx`: shift siblings for insert in transaction.
    - `move_category_in_tx`: reposition category in hierarchy.
    - `reorder_category_siblings_in_tx`: reorder sibling category placements.
  - **Category Localized Copy & Projections**:
    - `load_scoped_categories`: load localized category metadata snapshots for category lists, trees, and search projections.
    - `load_category_locale_copy_in_tx`: load localized category name/slug/description for existing category in transaction.
    - `sync_category_copy_in_tx`: synchronize created/updated category translations and hierarchy into taxonomy in transaction.
  - **Route Resolution**:
    - `resolve_category_route`: resolve category term route and alias by slug and locale.
  - **Tag Dictionary & Attachment**:
    - `resolve_term_names`: resolve term names for topic tags by tenant and locale.
    - `ensure_tags_in_tx`: ensure topic tag terms exist in taxonomy in transaction.
- Introduce `ForumTaxonomyConnection<'a>` supporting both `&DatabaseConnection` and `&DatabaseTransaction`.
- `CategoryService`, `ForumCategoryRouteService`, and `TopicService` accept an optional `Arc<dyn ForumTaxonomyPort>`.

### 2. Standalone Mode & Graceful Degradation
- Provide `NoopForumTaxonomyPort` for standalone microservice execution and unit/integration testing:
  - Category operations operate purely on local `forum_categories` rows.
  - Tag queries return fallback values without failing SQL queries.
  - No foreign table errors when `taxonomy_terms` is absent.

### 3. Concrete Provider Adapter in `rustok-taxonomy`
- Implement `TaxonomyForumTaxonomyPortAdapter` in `crates/modules/rustok-taxonomy/src/forum_taxonomy_port.rs`.
- `rustok-taxonomy` depends on `rustok-forum` (strictly acyclic: `taxonomy -> forum`).
- Remove `rustok-taxonomy` from `crates/modules/rustok-forum/Cargo.toml`.

### 4. Migration Independence
- Inline migration-specific entities and local helper functions in `m20260823_...` and `m20260824_...` so migration files do not import living `rustok_taxonomy` entities.

## Invariants

- `rustok-forum` contains zero direct SQL queries against `taxonomy_*` tables.
- `rustok-forum/Cargo.toml` contains no dependency on `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `taxonomy -> forum`.
- Standalone forum tests run against an isolated database without taxonomy tables.
- Transactional atomicity is fully preserved when `ForumTaxonomyPort` is wired with a database transaction.

## Non-goals

- Merging taxonomy storage or translations into forum entities.
- Removing taxonomy features or capabilities from the platform monolith.

## Verification

- `cargo check -p rustok-forum` passes without `rustok-taxonomy` in dependencies.
- `cargo test -p rustok-forum --lib` passes with decoupled ports.
- `cargo check -p rustok-taxonomy` passes with `ForumTaxonomyPort` adapter implemented.
