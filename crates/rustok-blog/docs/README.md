# `rustok-blog` Documentation

`rustok-blog` is the domain module for publication and comment scenarios on the blog
surface. The module already works on blog-owned persistence and uses shared
platform contracts only where justified by the responsibility boundary.

**Contract stability status:** fully achieved. Channel-aware semantics and
taxonomy sync are confirmed by integration and unit tests.

## Purpose

- publish the canonical blog runtime contract for posts, categories and tag relations;
- keep blog-owned transport surfaces, domain services and UI packages inside the module;
- evolve the blog as a channel-aware and taxonomy-aware domain without returning to shared storage.

## Scope

- `PostService`, `CommentService`, `CategoryService`, `TagService` and blog state machine;
- blog-owned storage for posts, translations, categories and typed relations;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages;
- REST post/comment handlers consume narrow `BlogHttpRuntime` state with explicit DB/event bus handles; `controllers::axum_router` builds that state from `HostRuntimeContext` and is mounted by generated host Axum composition without a framework adapter;
- moderation REST surface: `POST /api/blog/comments/{id}/moderate` for approve/spam/trash transitions with RBAC `blog_posts:manage`;
- channel visibility for publications and integration with `rustok-channel`;
- reuse shared taxonomy dictionary via `blog_post_tags`, without giving attachment ownership outward;
- observability via `rustok-telemetry`: `metrics::record_read_path_*` on GraphQL/REST read paths,
  `#[instrument]` on service methods, span tracking for post lifecycle and visibility filtering.

## Integration

- uses `rustok-taxonomy` as a shared vocabulary for tag identity;
- uses `rustok-comments` as a comment runtime contract;
- uses `rustok-profiles` for author presentation contract;
- uses `rustok-channel` for module-level and publication-level visibility on public read-path;
- uses `rustok-telemetry` for observability on read/write paths;
- `rustok-blog/admin` already embeds owner-side post SEO panel via `rustok-seo-admin-support`
  and the shared capability contract of the `rustok-seo` module.

## Contract Tests

Tests in `tests/contract_surface.rs` and `tests/integration.rs` cover:

- **Post lifecycle**: create → draft → publish → archive → restore
- **Locale fallback**: normalize → requested → en → first available
- **Channel visibility**: typed `blog_post_channel_visibility` allowlists, empty = global
- **Taxonomy sync**: blog tags ↔ `rustok-taxonomy` vocabulary
- **RBAC enforcement**: customer cannot create/read draft posts
- **GraphQL read paths**: public vs authenticated channel gating
- **Events**: blog.post.created/updated/published/archived/deleted/unpublished
- **Comments**: thread, locale fallback, status transitions, RBAC
- **State machine**: BlogPost status transitions, CommentStatus transitions

## Verification

- `cargo xtask module validate blog`
- `cargo xtask module test blog`
- targeted tests for post lifecycle, tag/category sync, channel visibility and public/admin read-path contracts

## Related documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [CRATE_API](../CRATE_API.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)

## FFA UI split

Leptos render adapters for admin and storefront live in `admin/src/ui/leptos.rs` and `storefront/src/ui/leptos.rs`; crate roots only connect module layers and re-export `BlogAdmin` / `BlogView`. Admin operations go through `admin/src/transport.rs`, while the storefront keeps native/GraphQL adapters behind a facade in `storefront/src/transport/`.
