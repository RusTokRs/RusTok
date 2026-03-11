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

### Phase 3 — Productionization (planned)

- [x] CommentService implementation
- [ ] CategoryService implementation
- [ ] TagService implementation
- [ ] RBAC enforcement: check permissions in service layer
- [ ] Rate limiting for post creation
- [ ] Full-text search integration via rustok-index
- [ ] Performance testing and optimization
- [ ] Integration tests with test database
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
| `services/category.rs` | ⬜ TODO | Blog categories |
| `services/tag.rs` | ⬜ TODO | Tag management |
| `dto/post.rs` | ✅ Complete | All fields, i18n, SEO, pagination |
| `entities/` | ✅ Complete | Re-exports from content module |
| Tests (unit) | ✅ Complete | State machine, DTOs, errors, locale, service |
| Tests (integration) | ⬜ TODO | Requires test database |
| Documentation | ✅ Complete | README, CRATE_API, docs |

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

2025-02-24 — Phase 2 complete: i18n, events, full fields, REST/GraphQL expansion
