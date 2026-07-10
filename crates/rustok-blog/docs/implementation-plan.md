# `rustok-blog` — Implementation Plan

Status: contract stability fully achieved; module moved into product
hardening and operability rollout. Channel-aware semantics and taxonomy sync confirmed
by integration and unit tests. GraphQL/REST adapters, Leptos admin/storefront
packages and module metadata synchronized.

## Execution checkpoint

- Current phase: axum_http_entrypoint
- Last checkpoint: `rustok-module.toml` declares `controllers::axum_router`, which builds `BlogHttpRuntime` from `HostRuntimeContext` plus its typed `TransactionalEventBus` handle. Generated host composition merges that router once and does not register a legacy Loco `Routes` entrypoint, so the blog domain crate no longer depends on `loco-rs`. Storefront FFA cleanup retired `storefront/src/api.rs`; native server-function code now lives in `storefront/src/transport/native_server_adapter.rs`, GraphQL selected-path code lives in `storefront/src/transport/graphql_adapter.rs`, and `verify-blog-storefront-boundary.mjs` rejects reintroducing the legacy API module. Public GraphQL/storefront reads now use `SecurityContext::public_read()` when `AuthContext` is absent, while published/channel-visible filters remain mandatory. Storefront native server functions now consume `HostRuntimeContext` plus a typed `TransactionalEventBus` host handle, no longer import Loco `AppContext`, and no longer depend on `loco-rs` or the outbox `loco-adapter` feature. FBA consumer runtime-order smoke now source-locks blog -> comments validation/provider-call/mapping order without compilation.
- Dependency evidence: storefront no-feature/hydrate profiles contain neither `rustok-core` nor `rustok-blog`; both backend crates are optional and enabled only by `ssr`.
- Previous checkpoint: FFA slice #103 moved admin posts-table CSS class selection into the Leptos-free `BlogPostAdminTableClassesViewModel` / `blog_post_admin_table_classes_view`, so table containers, headers, rows, cells and action button classes are prepared by core while the adapter keeps markup and event binding only.
- Next step: Continue small admin render/input fragments without changing the dual-path contract, or add real runtime contract execution against the comments port when compilation/runtime checks are allowed.
- Open blockers: None.
- Hand-off notes for next agent:
  1. Continue one-task-per-iteration: one helper/use-case -> storefront/admin -> docs double-check.
  2. Do not change the dual-path contract (`native #[server]` + GraphQL selected path) during FFA decomposition.
  3. After each slice, update parity evidence (`docs/verification/ffa-ui-parity-checklist.md`).
- Last updated at (UTC): 2026-06-30T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - public read authority slice: blog GraphQL request security resolves authenticated snapshots through `SecurityContext::from_permission_snapshot(...)` and resolves missing-auth public reads to `SecurityContext::public_read()`; storefront native selected-post reads use the same anonymous authority and keep published/channel visibility filtering in the module service path;
  - storefront/admin helper slices continue moving UI decision logic to `core` without changing the dual-path transport contract; storefront shell copy and selected-post route/query state now use framework-agnostic core view-model/state; storefront native and GraphQL transport paths are separated into explicit adapter modules; transport adapters consume core-owned fetch request state instead of raw UI tuples; admin calls now go through a module-owned `admin/src/transport.rs` facade instead of direct `api::*` calls from the Leptos adapter;
  - native `#[server]` + GraphQL remain parallel selected paths, GraphQL removal/replacement was not performed;
  - FBA consumer metadata and source-smoke evidence now lock the blog -> comments boundary: `crates/rustok-blog/contracts/blog-fba-registry.json` declares the `blog_post_comments` consumer profile for `CommentsThreadPort` / `comments.thread.v1`, `crates/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json` mirrors planned consumer contract cases/fallback degraded modes, `crates/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json` records no-compile source verification for embedded-native fallback/degraded-mode behavior, and `scripts/verify/verify-blog-fba.mjs` checks drift against `crates/rustok-comments/contracts/comments-fba-registry.json` plus service/error mapping source markers without long compilation;
  - consumer runtime-order smoke `crates/rustok-blog/contracts/evidence/blog-comments-consumer-runtime-order-smoke.json` and shared gate `scripts/verify/verify-consumer-fba-runtime-order.mjs` additionally lock `ensure_post_exists`/target ownership -> comments provider call -> typed mapping order for create/list/update/delete comment paths, plus typed comments error mapping markers; this is executable no-compile evidence and does not replace real runtime contract execution;
  - backend boundary currently works in in-process model; remote extraction readiness is conducted as an evolutionary track without ownership/contract change, and current source-smoke does not replace real runtime contract execution, therefore FBA status remains `in_progress`;
  - FFA storefront slice: `storefront/src/ui/leptos.rs` is an explicit render adapter, `storefront/src/transport/` owns the native/GraphQL adapters, legacy `storefront/src/api.rs` is removed, and `scripts/verify/verify-blog-storefront-boundary.mjs` locks the split. The native adapter now uses `HostRuntimeContext` plus typed DB/event-bus host handles, and the guardrail blocks Loco `AppContext`, `rustok_outbox::loco`, `loco-rs`, and `loco-adapter` regressions.
- Last verified at (UTC): 2026-07-08T00:00:00Z
- Owner: `rustok-blog` module team

## Scope of work

- maintain `rustok-blog` as an independent blog domain module;
- synchronize post/category/tag/comment contracts, UI packages and local docs;
- evolve channel-aware and taxonomy-aware semantics without returning to shared content storage;
- ensure observability for post lifecycle, visibility filtering and moderation flows.

## Current state

- blog posts, translations, categories and typed tag relations already live in module-owned storage;
- GraphQL/REST adapters and Leptos admin/storefront surfaces already live inside the module;
- comments runtime contract comes from `rustok-comments`, and author presentation from `rustok-profiles`;
- public read-path already supports module-level and publication-level channel visibility;
- `blog_post_channel_visibility` table implements typed channel allowlists;
- blog services re-validate RBAC locally for posts, categories and tags;
- customer read paths restricted to published posts;
- observability already partially implemented: `metrics::record_read_path_*` on GraphQL/REST read paths,
  `#[instrument]` on all service methods, span-tracking for post lifecycle;
- for storefront UI, the FFA core/transport/ui split is already extracted: formatting/fallback helper logic moved to `storefront/src/core.rs`, native/GraphQL adapters live in `storefront/src/transport/`, and the Leptos render adapter is in `storefront/src/ui/leptos.rs`; admin UI uses `admin/src/core.rs`, `admin/src/transport/mod.rs` facade and `admin/src/ui/leptos.rs`.

## Stages

### 1. Contract stability

- [x] close storage split and blog-owned transport boundary;
- [x] migrate tag vocabulary to shared `rustok-taxonomy`, keeping blog-owned attachments;
- [x] embed channel-aware public visibility contract;
- [x] maintain sync between runtime contracts, UI packages and module metadata.

### 2. Product hardening

- [ ] bring rate limiting and performance baseline for public/write paths;
  - infrastructure: `rustok-core::security::rate_limit::RateLimiter` exists (token bucket, IP/key-based);
  - task: wire `RateLimiter` into blog REST/GraphQL public endpoints via middleware.
- [ ] bring search/index integration without blurring blog domain boundary;
  - blog publishes domain events (`blog.post.created/updated/published/archived/deleted/unpublished`);
  - events already marked `affects_index() = true` — `rustok-index` consumer processes them;
  - task: ensure indexer correctly maps events to search schema (verify mapping in `rustok-index`).
- [x] maintain category/tag/comment semantics covered by targeted integration tests.
- [x] add moderation API endpoints for comment status transitions (approve/spam/trash).
  - REST endpoint `POST /api/blog/comments/{id}/moderate` added;
  - endpoint routed through `controllers/` and calls `CommentService::moderate_comment`;
  - moderation RBAC locked on `BLOG_POSTS_MANAGE`, status maps to `rustok_comments::CommentsService::set_comment_status`.

### 3. Operability

- [x] evolve observability for post lifecycle, visibility filtering and moderation flows;
  - `#[instrument]` on all service methods (`PostService`, `CategoryService`, `TagService`, `CommentService`);
  - `rustok_comments::CommentsService::set_comment_status` also has `#[instrument]` (fields: tenant_id, comment_id, status);
  - `metrics::record_read_path_*` on GraphQL/REST read paths;
  - state machine transitions logged via `tracing::info!` (Draft→Published, etc.);
  - `CommentStatus` transitions exist in `state_machine.rs` (`approve`, `mark_spam`, `trash`).
- [ ] document new public/runtime guarantees simultaneously with service changes;
- [x] keep local docs, README and manifest metadata synchronized.

## Verification

- [x] `cargo xtask module validate blog`
- [x] `cargo xtask module test blog`
- [x] targeted tests for lifecycle, taxonomy sync, channel visibility and UI-facing read contracts
- [x] contract tests cover all public use-cases

## Contract surface tests

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

## Update rules

1. When changing blog runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing dependency graph, UI wiring or metadata, synchronize `rustok-module.toml`.
4. When changing channel/tag semantics, also update related module docs and central references.
5. When adding new public use-cases, add corresponding contract test in `tests/contract_surface.rs`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.

## FFA migration status

The completed admin and storefront core/transport/ui extraction is documented in the module README and protected by `verify-blog-admin-boundary.mjs` and the storefront boundary verifier. The parallel native `#[server]` and GraphQL contract remains unchanged.

- [ ] Re-run `cargo xtask module validate blog` and `cargo xtask module test blog` after the next runtime-contract change.
