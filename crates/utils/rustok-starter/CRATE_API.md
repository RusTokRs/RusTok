# CRATE_API: rustok-starter

## 1. Architectural Role & Boundary Enforcement

`rustok-starter` owns declarative Starter Blueprint models and the multi-module data import engine for RusToK:
- **Declarative Blueprint Packages:** Blueprints describe seed content (`taxonomy`, `pages`, `blog`, `forum`, `navigation`) in a schema-versioned, human-readable structure.
- **Topological Import Orchestration:** Content is imported in strict topological dependency order (Taxonomy & Blog categories -> Landing pages -> Blog posts -> Forum categories & topics -> Navigation menus & default channel binding).
- **Domain Service Seams (No Raw SQL):** All mutations execute exclusively through public module application services (`PageService`, `BlogCategoryService`, `PostService`, `ForumCategoryService`, `TopicService`, `ReplyService`, `ModerationService`, `MenuService`, `MenuBindingService`), guaranteeing business invariant enforcement and transactional event emission.
- **Re-run Idempotency:** All drivers perform scoped lookups before insertion, skipping existing entities without error or duplicate domain records.
- **Tenant Scope Isolation:** All operations are strictly constrained to the caller-supplied `tenant_id` and verified security context.

---

## 2. Public Surface & Data Structures

### Primary Types & Entrypoints
- `StarterEngine`: Orchestrator driving blueprint imports into tenant workspaces.
  - `new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self`
  - `import_blueprint(&self, tenant_id: Uuid, security: &SecurityContext, blueprint: &StarterBlueprint) -> StarterResult<StarterExecutionReport>`
- `StarterBlueprint`: Typed blueprint definition package (`schema_version`, `id`, `name`, `description`, `locale`, `content`).
- `StarterContent`: Domain-grouped content payload (`taxonomy`, `pages`, `blog`, `forum`, `navigation`).
- `TaxonomyStarter`: Taxonomy categories and tags.
- `PageStarter`: Visual builder landing page definition (`slug`, `title`, `template`, `publish`, `channels`, `body: PageBodyStarter`).
- `BlogStarter`: Blog posts with categories, tags, rich-text markdown, and multi-locale translations.
- `ForumStarter`: Discourse-style forum categories and topics with tags, replies, and accepted solution flags.
- `NavigationStarter`: Navigation menu hierarchies and channel bindings.
- `StarterExecutionReport`: Structured execution telemetry (`tenant_id`, `blueprint_id`, `pages_created`, `blog_categories_created`, `blog_posts_created`, `forum_categories_created`, `forum_topics_created`, `forum_replies_created`, `menus_created`, `skipped_existing`, `duration_ms`).
- `default_starter() -> StarterBlueprint`: Returns the embedded production starter blueprint (`default-starter`).

---

## 3. Error Contracts & Edge Case Guarantees

- `StarterError`:
  - `Database(DbErr)`: Relational database failures.
  - `Json(serde_json::Error)`: Serialization / deserialization failures.
  - `Pages(PagesError)`: Pages domain validation or runtime errors.
  - `Blog(BlogError)`: Blog domain errors.
  - `Forum(ForumError)`: Forum domain errors.
  - `Navigation(NavigationError)`: Navigation domain errors.
  - `Validation(String)`: Blueprint validation failures.
  - `TenantNotFound(String)`: Specified tenant does not exist.
  - `BlueprintNotFound(String)`: Specified blueprint package cannot be resolved.
  - `Io(std::io::Error)`: Filesystem IO errors.
- **Pagination Boundary Compliance:** Blog category and forum queries request max bounded items (e.g., `per_page: 100`) to guarantee complete tenant inspection and eliminate pagination clipping during idempotency checks.
- **System Author Bootstrap:** When security context lacks an author ID (system context in CLI/installer), a deterministic bootstrap author ID is generated from `tenant_id`, ensuring valid author foreign keys.

---

## 4. Concurrency & Concurrency Models

- **Transactional Event Bus Integration:** Entity operations emit outbox events via `TransactionalEventBus`, ensuring transactional consistency between database mutations and outbox publishing.
- **Single Tenant Context:** Import is executed within the scope of a single tenant, with read checks and writes operating against the configured connection pool.
- **Safe Re-execution:** Blueprints can be safely executed multiple times concurrently or sequentially against the same tenant; existing entities are skipped and reported in `skipped_existing`.

---

## 5. Audit History & Verification Evidence

- **Phase FS-09 Component Audit (2026-10-31):**
  - Standardized `Cargo.toml` (`version.workspace = true`, `edition.workspace = true`, `license.workspace = true`, `rust-version.workspace = true`, `readme = "README.md"`, `repository.workspace = true`, `[package.metadata.docs.rs]`).
  - Identified and resolved pagination clipping bug in `rustok-blog` where derived `Default` for `ListCategoriesFilter`, `ListTagsFilter`, and `ListCommentsFilter` caused `per_page = 0` to clamp to `1`, resulting in duplicate slug conflicts during re-run.
  - Verified 6 Universal Logic Dimensions across blueprint deserialization, driver execution, and error conversions.
  - Authored comprehensive `contract_tests.rs` and verified integration tests:
    - `cargo test -p rustok-starter --lib` (unit & contract tests)
    - `cargo test -p rustok-starter --test starter_import_integration`
  - Clippy verification: 0 warnings with `-D warnings`.

