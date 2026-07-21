# `rustok-blog` Documentation

`rustok-blog` is the domain module for publication, Blog category, and comment
scenarios. The module owns its persistence and uses shared platform contracts
only across explicit boundaries.

All Blog comment lifecycle operations consume the public `CommentsThreadPort`
with typed actor, locale, deadline, idempotency where required, and error
semantics. Comments lifecycle events are consumed by Blog's durable idempotent
reply-count projection, which publishes `BlogPostUpdated` in the same projection
transaction.

## Purpose

- publish the canonical Blog runtime contract for posts, categories, and tag relations;
- keep Blog-owned transport surfaces, domain services, and UI packages inside the module;
- evolve the Blog as a channel-aware and taxonomy-aware domain without shared storage;
- expose distinct `blog_posts:*` and `blog_categories:*` authority resources.

## Scope

- `PostService`, `CommentService`, `CategoryService`, `TagService`, and the Blog state machine;
- Blog-owned storage for posts, translations, categories, and typed relations;
- GraphQL, REST, Leptos admin, and storefront transport surfaces;
- REST handlers consume narrow `BlogHttpRuntime` state with explicit DB/event bus handles; `controllers::axum_router` builds that state from `HostRuntimeContext`;
- category REST CRUD under `/api/blog/categories` requires `blog_categories:*`;
- `CategoryService::new(db, event_bus)` is the only category service constructor;
- category update/delete and tenant Blog-scope reindex publication share one transaction;
- moderation REST surface `POST /api/blog/comments/{id}/moderate` uses `blog_posts:manage`;
- channel visibility for publications and integration with `rustok-channel`;
- shared taxonomy dictionary reuse via `blog_post_tags`, without transferring attachment ownership;
- observability via `rustok-telemetry` read-path metrics and instrumented service methods.

## Permission boundary

`Resource::BlogCategories` serializes as `blog_categories`. Built-in roles,
public-read authority, OAuth content scopes, module permission registration,
HTTP preflight, and owner services use this resource. Catalog `categories:*`
and post `blog_posts:*` permissions do not grant Blog category access.

## Integration

- uses `rustok-taxonomy` as a shared vocabulary for tag identity;
- uses `rustok-comments` as a comment runtime contract;
- uses `rustok-profiles` for author presentation;
- uses `rustok-channel` for module-level and publication-level public visibility;
- uses `rustok-telemetry` for read/write observability;
- `rustok-blog/admin` embeds the owner-side post SEO panel through the shared `rustok-seo` capability contract.

## Contract Tests

Tests in `tests/contract_surface.rs`, `tests/module.rs`, and `tests/integration.rs` cover:

- **Post lifecycle**: create → draft → publish → archive → restore
- **Locale resolution**: normalize → requested → en → first available
- **Channel visibility**: typed `blog_post_channel_visibility` allowlists, empty = global
- **Taxonomy sync**: Blog tags ↔ `rustok-taxonomy` vocabulary
- **RBAC enforcement**: distinct post/category resources and denied cross-resource grants
- **Category invariants**: mandatory event bus, tenant parent/translation scope, slug validation, pagination cap
- **GraphQL read paths**: public vs authenticated channel gating
- **Events**: Blog post lifecycle and category-triggered Search reindex
- **Comments**: thread, locale resolution, status transitions, RBAC
- **State machine**: BlogPost and CommentStatus transitions

## Verification

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- `node scripts/verify/verify-blog-category-search-reindex.mjs`
- targeted tests for lifecycle, category authority, outbox rollback, Search refresh, channel visibility, and public/admin read paths

## Related documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)

## FFA UI split

Leptos render adapters for admin and storefront live in `admin/src/ui/leptos.rs`
and `storefront/src/ui/leptos.rs`. Crate roots connect module layers and
re-export `BlogAdmin` / `BlogView`. Admin operations go through
`admin/src/transport.rs`; storefront native and GraphQL adapters remain behind
the storefront transport facade.
