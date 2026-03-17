# rustok-blog module implementation plan (`rustok-blog`)

## Scope and objective

This document captures the current implementation plan for `rustok-blog` in RusToK and
serves as the source of truth for rollout sequencing in `crates/rustok-blog`.

Primary objective: evolve `rustok-blog` in small, testable increments while preserving
compatibility with platform-level contracts.

## Target architecture

- `rustok-blog` remains focused on its bounded context and public crate API.
- Integrations with other modules go through stable interfaces in `rustok-core`
  (or dedicated integration crates where applicable).
- Behavior changes are introduced through additive, backward-compatible steps.
- Observability and operability requirements are part of delivery readiness.

## Delivery phases

### Phase 0 — Foundation ✅ DONE

- [x] Baseline crate/module structure is in place.
- [x] Base docs and registry presence are established.
- [x] Core compile-time integration with the workspace is available.
- [x] Module metadata (slug, name, description, version).
- [x] Empty migrations (wrapper module).

### Phase 1 — Contract hardening ✅ DONE

- [x] Freeze public API expectations for the current module surface.
- [x] Align error/validation conventions with platform guidance.
  - [x] `BlogError` with RichError conversion
  - [x] Helper methods for common errors
- [x] Expand automated tests around core invariants and boundary behavior.
  - [x] Unit tests for state machine
  - [x] Property-based tests for state machine
  - [x] Module metadata tests
  - [x] DTO validation tests
- [x] Define permissions for all resources.
  - [x] Posts (CRUD + Publish)
  - [x] Comments (CRUD + Moderate)
  - [x] Categories (CRUD)
  - [x] Tags (CRUD)
- [x] Type-safe state machine implementation.
  - [x] Draft, Published, Archived states
  - [x] Compile-time safe transitions
  - [x] Comment status with transitions

### Phase 2 — I18n + Events + Full Fields ✅ DONE

- [x] Locale fallback chain: requested → en → first available
  - [x] `locale.rs` module with `resolve_translation()`, `resolve_body()`, `available_locales()`
  - [x] 8 unit tests for all fallback branches
- [x] New DTO fields:
  - [x] `effective_locale` — actual locale used
  - [x] `available_locales` — all available locales (PostResponse)
  - [x] `featured_image_url` — cover image URL
  - [x] `seo_title` — SEO title
  - [x] `seo_description` — SEO meta description
  - [x] `body_format` — format of body (markdown/html)
  - [x] `category_id` in PostSummary
- [x] Blog-specific DomainEvents:
  - [x] `BlogPostCreated { post_id, author_id, locale }`
  - [x] `BlogPostPublished { post_id, author_id }`
  - [x] `BlogPostUnpublished { post_id }`
  - [x] `BlogPostUpdated { post_id, locale }`
  - [x] `BlogPostArchived { post_id, reason }`
  - [x] `BlogPostDeleted { post_id }`
  - [x] Full `event_type()`, `schema_version()`, `affects_index()`, `validate()` support
- [x] PostService full implementation:
  - [x] `author_id` from SecurityContext
  - [x] `get_post(post_id, locale)` with locale resolution
  - [x] `list_posts()` fetches full nodes for correct metadata
  - [x] `update_post()` with tenant_id from node
  - [x] `publish_post()`, `unpublish_post()`, `archive_post()` with events
  - [x] `delete_post()` with CannotDeletePublished guard
- [x] REST API expanded:
  - [x] `GET /api/blog/posts?locale=ru` with PostListQuery
  - [x] `GET /api/blog/posts/:id?locale=ru` with locale param
  - [x] `POST /api/blog/posts/:id/publish`
  - [x] `POST /api/blog/posts/:id/unpublish`
- [x] GraphQL API expanded:
  - [x] `publishPost`, `unpublishPost`, `archivePost` mutations
  - [x] `UpdatePostInput` uses blog-native DTO (not content UpdateNodeInput)
  - [x] `CreatePostInput` includes SEO and featured_image fields
- [x] Swagger updated with all new types

### Phase 3 — Productionization (in progress)

- [x] CommentService implementation
- [x] CategoryService implementation (`services/category.rs` — full CRUD, tenant isolation, slug auto-generation)
- [x] TagService implementation (`services/tag.rs` — full CRUD, slug normalization)
- [x] Integration tests with test database (all 5 post lifecycle tests + 2 new category/tag tests now green)
- [x] `category` and `tag` kinds registered in content module validation and RBAC
- [x] In-memory tag filtering in `PostService::list_posts` (pre-index fallback)
- [ ] RBAC enforcement: check permissions in service layer
- [ ] Rate limiting for post creation
- [ ] Full-text search integration via rustok-index (will supersede in-memory tag filtering)
- [ ] Performance testing and optimization
- [ ] View counter via redis/atomic increment
- [ ] category_name denormalization in list responses

## Current Status

| Component | Status | Notes |
|-----------|--------|-------|
| `lib.rs` | ✅ Complete | Module definition, permissions, exports |
| `error.rs` | ✅ Complete | BlogError with RichError conversion |
| `locale.rs` | ✅ Complete | Fallback chain, 8 unit tests |
| `state_machine.rs` | ✅ Complete | Type-safe post states, comment status |
| `state_machine_proptest.rs` | ✅ Complete | Property-based tests |
| `services/post.rs` | ✅ Complete | Full CRUD + i18n + events |
| `services/comment.rs` | ✅ Complete | CRUD + locale fallback + threaded comments |
| `services/category.rs` | ✅ Complete | CRUD + tenant isolation + slug auto-generation |
| `services/tag.rs` | ✅ Complete | CRUD + slug normalization |
| `dto/post.rs` | ✅ Complete | All fields, i18n, SEO, pagination |
| `entities/` | ✅ Complete | Re-exports from content module |
| Tests (unit) | ✅ Complete | State machine, DTOs, errors, locale, service |
| Tests (integration) | ✅ Complete | All lifecycle and category/tag tests green; 18 integration tests pass |
| Documentation | ✅ Complete | README, CRATE_API, docs |

## Open TODO decomposition

### `services/category.rs` — ✅ DONE (Phase 3 / Priority: P1)

**Scope of work**
- Add `CategoryService` with CRUD + list API (`create_category`, `get_category`, `update_category`, `delete_category`, `list_categories`) aligned with blog tenancy model.
- Define category DTOs in `dto/category.rs` and export from crate root.
- Integrate with `PostService` validation to reject non-existing `category_id` at create/update time.
- Provide REST/GraphQL endpoints after service readiness (same release phase).

**Dependencies (data/migrations/API)**
- Data model source: `rustok-content` entities (`nodes` + metadata) and/or dedicated category storage decision.
- Migrations: if category gets dedicated table, add migration in content/blog migration chain; if metadata-based, no schema migration but strict metadata contract update.
- API dependencies: synchronized changes in server handlers and GraphQL schema for category management.

**Definition of Done**
- `CategoryService` implemented and exported via `services/mod.rs` and `lib.rs`.
- All methods enforce tenant isolation and return `BlogError::CategoryNotFound` where applicable.
- Post create/update validates `category_id` existence.
- REST + GraphQL contracts updated and documented.

**Test scenarios**
- Unit: create/get/update/delete/list happy paths.
- Unit: category not found, duplicate slug/name handling, cross-tenant access forbidden.
- Integration: post creation with valid category succeeds; with missing category fails.

**Key implementation references (current related code)**
- Error contract already present: [`src/error.rs`](../src/error.rs).
- Category identifier already present in post DTO/service payloads: [`src/dto/post.rs`](../src/dto/post.rs), [`src/services/post.rs`](../src/services/post.rs).

### `services/tag.rs` — ✅ DONE (Phase 3 / Priority: P1)

**Scope of work**
- Add `TagService` with CRUD/list + optional slug normalization.
- Define DTOs in `dto/tag.rs` and export from public API.
- Connect `PostService` validation to ensure submitted tags exist or are auto-created (strategy to be fixed before implementation).
- Add API surface for tag discovery/management.

**Dependencies (data/migrations/API)**
- Data: resolve canonical tag storage (`nodes` metadata vs dedicated table) and uniqueness constraints.
- Migrations: required if dedicated storage/unique index is introduced.
- API: REST and GraphQL query/mutation additions for tags; docs + OpenAPI sync.

**Definition of Done**
- `TagService` implemented and exported.
- Tag lifecycle covered by tenant-aware validation and deterministic normalization.
- Post flows either validate-only or upsert tags according to approved policy.
- API + docs fully synchronized.

**Test scenarios**
- Unit: create/list/update/delete tags, normalization, duplicate prevention.
- Unit: tenant isolation and `TagNotFound` mapping.
- Integration: filter posts by tag with persisted tag entities.

**Key implementation references (current related code)**
- Tag-related errors already defined: [`src/error.rs`](../src/error.rs).
- Existing post tag flow + filtering API: [`src/services/post.rs`](../src/services/post.rs), [`src/dto/post.rs`](../src/dto/post.rs).

### Tests (integration) — ✅ DONE (Phase 3 / Priority: P0)

**Actual status in code**
- File `tests/integration.rs` already includes working sqlite-backed integration coverage for comment/thread workflows and event checks.
- Several post lifecycle tests remain `#[ignore]` and currently act as placeholders for full DB/migration/indexer wiring.

**Scope of remaining work**
- Unignore and complete post lifecycle integration tests (`create → publish → list/filter → delete guards`).
- Wire test bootstrap to real migration path instead of ad-hoc schema SQL.
- Add assertions for emitted blog domain events and permission checks.

**Dependencies (data/migrations/API)**
- Stable test DB bootstrap (sqlite or postgres) with deterministic migration execution.
- Optional index/outbox test harness when validating publish/index flows.

**Definition of Done**
- Ignored integration tests converted into green, deterministic CI-suitable tests (or split into explicit nightly profile with clear policy).
- Coverage includes post lifecycle, list filtering by tag/category, forbidden mutations, and publish/delete invariants.

**Test scenarios**
- `test_create_and_publish_post`: status transitions + publish timestamps.
- `test_list_posts_with_pagination` and `test_filter_posts_by_tag`: pagination and filtering correctness.
- `test_cannot_delete_published_post`: invariant enforcement.

**Key implementation references**
- Integration test suite: [`tests/integration.rs`](../tests/integration.rs).
- Current service behavior under test: [`src/services/post.rs`](../src/services/post.rs), [`src/services/comment.rs`](../src/services/comment.rs).

## Module Contracts

### Public API

```rust
pub use dto::{
    CreatePostInput, PostListQuery, PostListResponse,
    PostResponse, PostSummary, UpdatePostInput
};
pub use error::{BlogError, BlogResult};
pub use locale;
pub use services::PostService;
pub use state_machine::{
    Archived, BlogPost, BlogPostStatus, CommentStatus, Draft, Published, ToBlogPostStatus,
};
```

### Permissions

```rust
Permission::new(Resource::Posts, Action::Create);
Permission::new(Resource::Posts, Action::Read);
Permission::new(Resource::Posts, Action::Update);
Permission::new(Resource::Posts, Action::Delete);
Permission::new(Resource::Posts, Action::List);
Permission::new(Resource::Posts, Action::Publish);
Permission::new(Resource::Comments, Action::Create);
Permission::new(Resource::Comments, Action::Read);
Permission::new(Resource::Comments, Action::Update);
Permission::new(Resource::Comments, Action::Delete);
Permission::new(Resource::Comments, Action::List);
Permission::new(Resource::Comments, Action::Moderate);
// Categories & Tags: standard CRUD
```

### State Transitions

| From | To | Method | Allowed On |
|------|-----|--------|------------|
| Draft | Published | `publish_post()` | Draft |
| Published | Archived | `archive_post(reason)` | Published |
| Published | Draft | `unpublish_post()` | Published |
| Archived | Draft | `restore_to_draft()` | Archived (state machine only) |

### Events

| Event | When | Affects Index |
|-------|------|---------------|
| `BlogPostCreated` | create_post() | ✅ |
| `BlogPostPublished` | publish_post() | ✅ |
| `BlogPostUnpublished` | unpublish_post() | ✅ |
| `BlogPostUpdated` | update_post() | ✅ |
| `BlogPostArchived` | archive_post() | ✅ |
| `BlogPostDeleted` | delete_post() | ✅ |

## Tracking and updates

When updating `rustok-blog` architecture, API contracts, tenancy behavior, routing,
or observability expectations:

1. Update this file first.
2. Update `crates/rustok-blog/README.md` and `crates/rustok-blog/docs/README.md` when public behavior changes.
3. Update `docs/index.md` links if documentation structure changes.
4. If module responsibilities change, update `docs/modules/registry.md` accordingly.

## Last Updated

2026-03-17 — Phase 3: CategoryService, TagService implemented; all integration tests green; tag/category kinds registered in content validation and RBAC


### Rich-text admin integration (update)

- [x] Integration in admin package and GraphQL surfaces (post form + PageBuilder + ForumReplyEditor exports).

- [x] Navigation entry points split by domain: blog workflows stay under Blog menu, forum workflows moved to Forum menu (`/dashboard/forum/reply`).

## Checklist

- [x] контрактные тесты покрывают все публичные use-case.

