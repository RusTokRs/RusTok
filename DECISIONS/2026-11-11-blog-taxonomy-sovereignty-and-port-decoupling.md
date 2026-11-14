# Blog and Taxonomy storage sovereignty and Blog port decoupling

- Date: 2026-11-11
- Decision status: Accepted
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

### 3. Acyclic Architecture & Host Wiring
- `BlogTaxonomyPort` and `NoopBlogTaxonomyPort` are defined within `rustok-blog::ports::taxonomy`.
- Because `rustok-profiles` depends on `rustok-taxonomy`, and `rustok-blog` depends on `rustok-profiles`, `rustok-taxonomy` MUST NOT declare a compile-time Cargo dependency on `rustok-blog` to avoid cyclic path dependencies (`blog -> profiles -> taxonomy -> blog`).
- The host runtime context dynamically injects `Arc<dyn BlogTaxonomyPort>` via host capabilities, defaulting gracefully to `NoopBlogTaxonomyPort` when absent.
- Remove `rustok-taxonomy` from `crates/modules/rustok-blog/Cargo.toml` `[dependencies]`.

### 4. Route Slug Normalization Independence
- Replace `rustok_taxonomy::normalize_term_route_key` calls in `rustok-blog` with local slugification using `slug::slugify`.

### 5. Translation Owner & Category Cleanup Abstraction
- Define `BlogCategoryDeleteCleanupPort` inside `rustok-blog::ports::taxonomy`.
- Relocate or adapt the translation owner registration so `rustok-blog` does not depend on `rustok-taxonomy`.

## Sources of truth and ownership

- `rustok-taxonomy` is the single source of truth for the `taxonomy_*` tables, term names, term routes and the category tree.
- `rustok-blog` owns `blog_posts`, `blog_post_tags`, `blog_tag_usage`, `blog_categories` and the command rules that refuse a Blog write.
- `rustok-blog` owns `BlogTaxonomyPort` and `BlogCategoryDeleteCleanupPort` in `rustok-blog::ports::taxonomy`; the provider owns the adapter.
- `rustok-blog` keeps one Taxonomy-shaped dependency for the translation-owner registry (`TaxonomyModuleTermTranslationOwnerRegistry`, `TaxonomyModuleTermTranslationOwner`, `TaxonomyTermKind` in `src/module.rs` and `src/services/taxonomy_translation_owner.rs`). That registry is a registration point, not a data read; step 5 of the decision moves it, and until then the Cargo dependency of Context 2 stands.
- The composition root (`apps/server`, the module registry) owns which port is attached.

## Invariants

- `rustok-blog` contains zero direct SQL queries against `taxonomy_*` tables.
- `rustok-blog/Cargo.toml` contains no dependency on `rustok-taxonomy`.
- The Cargo dependency graph is strictly acyclic: `taxonomy -> blog`.
- Standalone blog tests run against an isolated database without taxonomy tables.
- Transactional atomicity is fully preserved when `BlogTaxonomyPort` is wired with a database transaction.

## Non-goals

- Merging taxonomy storage or translations into blog entities.
- Removing taxonomy features or capabilities from the platform monolith.

## Data, transaction, and concurrency boundary

- Every `*_in_tx` operation of the port runs on the `&DatabaseTransaction` of the Blog write, so in the monolith the Blog row and the Taxonomy copy commit or roll back together.
- `BlogTaxonomyConnection<'a>` is either `&DatabaseConnection` or `&DatabaseTransaction`; a read assembled inside a write can take the write's transaction, and `TagService::list_tags` reads valid tag identifiers through the transaction it opens.
- Taxonomy decides whether the transaction it is handed is enough: `lock_category_hierarchy_in_tx`, `lock_tag_in_tx` and the placement mutations take the lock they need, and a provider refusal is a `BlogError` that fails the Blog operation rather than a best-effort warning.
- `load_category_locale_copy_in_tx` and `sync_category_copy_in_tx` are the only path by which a Category copy reaches Taxonomy; Blog does not write a localized category row of its own.
- No schema, data or route change is part of this decision: the `taxonomy_terms` foreign keys that `blog_post_tags` and `blog_tag_usage` declare are removed by the follow-up migration work, not by this ADR.

## Context dimensions

- Tenant: every port operation takes `tenant_id`; `load_scoped_categories`, `load_scoped_tags` and `find_all_valid_tag_ids` are tenant-scoped reads.
- Locale: `load_scoped_categories`, `load_tags_by_ids` and `load_category_locale_copy_in_tx` carry a locale, and a snapshot states the locale it was asked for and the locale it returned so Blog can reject a mismatched copy instead of substituting one.
- Channel: Blog's channel gating stays in Blog; the port carries no channel dimension.
- Actor: the command operations of `CategoryCommandService` carry the caller's `SecurityContext` and enforce the Blog scope before the port is called.

## Events and projections

- The decision adds no event. Taxonomy term lifecycle events stay with Taxonomy; Blog post and category publication events keep their current shape.
- Search projection reads tag names through `load_scoped_tags` rather than from `taxonomy_terms`, so a deployment without a Taxonomy adapter indexes raw ids or slugs instead of failing.
- Traffic-side projections (tag listings, category lists) read through `load_tags_by_ids` and `load_scoped_categories`; a refusal or a missing adapter is a readable result, not a broken page.

## Failure semantics

- No adapter attached: `NoopBlogTaxonomyPort` answers reads with empty or locally-derived values and accepts writes as no-ops, so a standalone blog runs on a Blog-only schema. Tag creation in that mode stores nothing in Taxonomy and returns a generated identifier, which is why the README records it as open rather than complete.
- Adapter attached and the provider refuses: the refusal reaches the caller as a `BlogError` and the operation is not committed. A malformed snapshot (missing locale) is rejected the same way and rolls the update back.
- A Blog Category delete without a `BlogCategoryDeleteCleanupPort` leaves the canonical term in place; the port is what asks Taxonomy to clean up.
- Not decided here: whether a missing adapter should fail closed for write paths the way the pricing decision does. Today it does not.

## Migration and cutover

1. Define `BlogTaxonomyPort` and `BlogCategoryDeleteCleanupPort` in `rustok-blog::ports::taxonomy`.
2. Move every Taxonomy call in `rustok-blog` behind the ports (`category.rs`, `category_command.rs`, `category_delete.rs`, `category_name_projection.rs`, `tag.rs`, search projection, HTTP and GraphQL runtime).
3. Implement `TaxonomyBlogTaxonomyPortAdapter` and the cleanup adapter in `rustok-taxonomy`.
4. Attach both adapters in the composition root.
5. Remove `rustok-taxonomy` from `crates/modules/rustok-blog/Cargo.toml` by moving the translation-owner registration point.

Implementation state, read from the tree at 2026-11-12: steps 1 and 2 are present — the two ports exist, `NoopBlogTaxonomyPort` and `NoopBlogCategoryDeleteCleanupPort` are the defaults of the HTTP and GraphQL runtimes, `normalize_term_route_key` is Blog-local (`src/normalization.rs`), and Blog `src` holds no query against a `taxonomy_*` table. Steps 3, 4 and 5 are not present: no `TaxonomyBlogTaxonomyPortAdapter` exists anywhere in the tree, so both transports run on the noop port, and `crates/modules/rustok-blog/Cargo.toml` still declares `rustok-taxonomy` because `src/module.rs` and `src/services/taxonomy_translation_owner.rs` register a translation owner through it. Two consequences are recorded in `crates/modules/rustok-blog/README.md`: a Category or Tag written today is not stored in Taxonomy, and `tests/category_taxonomy_delete_lifecycle.rs` does not compile against the port signature. The invariant "`rustok-blog` contains no dependency on `rustok-taxonomy`" therefore does not hold yet, which is why the implementation status is `In progress`.

## Alternatives considered

- Keeping the direct dependency and calling `TaxonomyService` from Blog: rejected for the reasons in Context — a standalone blog cannot start on a Blog-only schema, and `TagService::list_tags` reads a table it does not own.
- Moving the taxonomy tables into Blog: rejected as a Non-goal; other modules place their own terms in the same tree.
- Leaving tag names unreadable in standalone mode instead of returning raw ids: rejected as a worse storefront result, and the port makes the degradation explicit.

## Consequences

- `rustok-taxonomy` gains an adapter crate dependency on `rustok-blog`; the dependency is strictly acyclic (`taxonomy -> blog`).
- A deployment that attaches no adapter keeps working and stores nothing in Taxonomy for Blog categories and tags; the README states this as open work rather than as a supported mode.
- Category copies, term routes and the category tree now follow the port, so Taxonomy can change its storage without a Blog change.
- The translation-owner registration keeps one Taxonomy type in Blog `src` until step 5, so `rustok-blog` is not yet free of the dependency its Context objects to.

## Verification

- Current state: these Rust checks have not been run in this environment because the Rust toolchain is unavailable.
- Before advancing the implementation status, run `cargo check -p rustok-blog` and `cargo test -p rustok-blog --lib` against the current ports and transports; the manifest still has a production `rustok-taxonomy` dependency, and the adapter, host wiring and translation-owner cutover are not implemented.
- After those cutovers, run `cargo check -p rustok-taxonomy` and adapter-backed integration tests that prove category/tag persistence, transaction rollback and standalone behavior. Do not claim a dependency-free Blog build until the manifest and code actually satisfy that invariant.

