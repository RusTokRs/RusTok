# Blog and Taxonomy storage sovereignty and Blog port decoupling

- Date: 2026-11-11
- Decision status: Proposed
- Implementation status: In progress
- Owners: rustok-blog, rustok-taxonomy
- Extends: [2026-09-18-canonical-native-module-reference-contract.md](./2026-09-18-canonical-native-module-reference-contract.md), [2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md](./2026-11-11-inventory-taxonomy-sovereignty-and-product-decoupling.md)
- Supersedes: None
- Superseded by: None

## Context

Following the decoupling of `rustok-product` from `rustok-taxonomy` via `ProductTaxonomyPort`, an architectural review of `rustok-blog` revealed that Blog remains tightly coupled to `rustok-taxonomy`:

1. **Direct Foreign SQL & Subquery Coupling**:
   - `TagService::list_tags` executes direct SQL subqueries against the `taxonomy_terms` table (`Query::select().column(rustok_taxonomy::entities::taxonomy_term::Column::Id).from(TaxonomyTerm)...`), directly breaching private module table boundaries.
   - `blog_post_tags` and `blog_tag_usage` schema migrations declare foreign key constraints to `taxonomy_terms`, creating physical database coupling.

2. **Direct Service, Reader & Transaction Coupling**:
   - `CategoryService`, `CategoryCommandService`, `CategoryDeleteCleanup`, and `category_taxonomy_sync` directly invoke `TaxonomyOwnerCategoryReader`, `TaxonomyService`, `sync_module_category_with_owned_aliases_in_tx`, `lock_category_hierarchy_writer_in_tx`, `move_module_category_in_tx`, `reorder_module_category_siblings_in_tx`, and `delete_module_category_placement_and_compact_in_tx`.
   - `SearchProjection` calls `TaxonomyOwnerReader` directly to build search documents.
   - `post/mod.rs` and `category.rs` call `rustok_taxonomy::normalize_term_route_key`.
   - `BlogError` contains `impl From<rustok_taxonomy::TaxonomyError> for BlogError`.
   - `lib.rs` and HTTP/GraphQL controllers re-export and require `rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort`.
   - `crates/modules/rustok-blog/Cargo.toml` declares a mandatory dependency on `rustok-taxonomy`.

3. **Impact on Fluid Backend Architecture (FBA) & Microservice Sovereignty**:
   - A standalone blog service or isolated test harness cannot start or execute read/write operations against an isolated database schema containing only `blog_posts`, `blog_categories`, and `blog_post_tags`.
   - Any query or mutation triggers missing-table errors when `taxonomy_terms` is absent.
   - This violates `AGENTS.md` §5 (module sovereignty, zero cross-module private table reads, no shadow/coupled dependencies).

## Decision

Apply the Ports & Adapters (Hexagonal / FBA) architecture pattern to decouple `rustok-blog` from `rustok-taxonomy`:

### 1. Consumer-Owned Port (`BlogTaxonomyPort`)
- Define `BlogTaxonomyPort` in `rustok-blog::ports::taxonomy`.
- The port specifies all taxonomy operations required during the blog category and tag lifecycle:
  - **Category Hierarchy & Copies**:
    - `lock_category_hierarchy_in_tx`: lock category hierarchy writer inside transaction.
    - `load_scoped_categories`: load localized category metadata snapshots for category lists and post projections.
    - `load_category_locale_copy_in_tx`: load localized category name/slug/description for existing category in transaction.
    - `sync_category_copy_in_tx`: synchronize created/updated category translations and hierarchy into taxonomy in transaction.
    - `move_category_in_tx`: reposition category in hierarchy.
    - `reorder_category_siblings_in_tx`: reorder sibling category placements.
    - `delete_category_placement_and_compact_in_tx`: remove category placement and compact siblings in transaction.
    - `delete_module_category_with_cleanup`: delete canonical category term with host cleanup callback.
  - **Tag Dictionary & Attachment**:
    - `load_tags_by_ids`: load localized tag snapshots for posts and tag listings.
    - `load_scoped_tags`: load scoped tags for search projection assembly.
    - `filter_valid_tag_ids`: verify which tag IDs are active without direct SQL joins on `taxonomy_terms`.
    - `create_tag_in_tx`: author new tag term in taxonomy in transaction.
    - `update_tag_in_tx`: update tag term in taxonomy in transaction.
    - `delete_tag_in_tx`: delete tag term in taxonomy in transaction.
    - `lock_tag_in_tx`: lock tag term for update in transaction.
- Introduce `BlogTaxonomyConnection<'a>` supporting both `&DatabaseConnection` and `&DatabaseTransaction`.
- `CategoryService`, `CategoryCommandService`, and `TagService` accept an optional `Arc<dyn BlogTaxonomyPort>`.

### 2. Standalone Mode & Graceful Degradation
- Provide `NoopBlogTaxonomyPort` for standalone microservice execution and unit/integration testing:
  - Category operations operate purely on local `blog_categories` rows.
  - Tag queries return fallback values without failing SQL queries.
  - No foreign table errors when `taxonomy_terms` is absent.

### 3. Concrete Provider Adapter in `rustok-taxonomy`
- Implement `TaxonomyBlogTaxonomyPortAdapter` in `crates/modules/rustok-taxonomy/src/blog_taxonomy_port.rs`.
- `rustok-taxonomy` depends on `rustok-blog` (strictly acyclic: `taxonomy -> blog`).
- Remove `rustok-taxonomy` from `crates/modules/rustok-blog/Cargo.toml`.

### 4. Route Slug Normalization Independence
- Replace `rustok_taxonomy::normalize_term_route_key` calls in `rustok-blog` with local slugification using `slug::slugify`.

### 5. Translation Owner & Category Cleanup Abstraction
- Define `BlogCategoryDeleteCleanupPort` inside `rustok-blog::ports::taxonomy`.
- Relocate or adapt the translation owner registration so `rustok-blog` does not depend on `rustok-taxonomy`.

## Invariants

- `rustok-blog` contains zero direct SQL queries against `taxonomy_*` tables.
- `rustok-blog/Cargo.toml` contains no dependency on `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `taxonomy -> blog`.
- Standalone blog tests run against an isolated database without taxonomy tables.
- Transactional atomicity is fully preserved when `BlogTaxonomyPort` is wired with a database transaction.

## Non-goals

- Merging taxonomy storage or translations into blog entities.
- Removing taxonomy features or capabilities from the platform monolith.

## Verification

- `cargo check -p rustok-blog` passes without `rustok-taxonomy` in dependencies.
- `cargo test -p rustok-blog --lib` passes with decoupled ports.
- `cargo check -p rustok-taxonomy` passes with `BlogTaxonomyPort` adapter implemented.

