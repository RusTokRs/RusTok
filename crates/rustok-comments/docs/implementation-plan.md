# Implementation plan for `rustok-comments`

This document captures the local roadmap of the `rustok-comments` module.

## Execution checkpoint

- Current phase: FBA provider baseline for generic comment threads
- Last checkpoint: Loco-free native admin transport now consumes host-provided `rustok_api::HostRuntimeContext` for DB access, and `rustok-comments-admin` no longer depends on `loco-rs`; comments port already applies canonical `PortCallPolicy`, then builds `SecurityContext` through strict `try_from_port_context`.
- Next step: Close runtime contract execution/fallback smoke for `CommentsThreadPort` and confirm blog embedded/native compatibility snapshots; for FFA, keep the native-only admin exception without new legacy/headless contract while maintaining Loco-free parity/evidence guardrails.
- Open blockers: none; native-only comments admin exception is locked because the module had no legacy GraphQL/REST admin surface.
- Hand-off notes for next agent: After each FFA/FBA increment, update this block, local FFA/FBA status block and central readiness board in the same PR.
- Last updated at (UTC): 2026-06-19T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-comments/contracts/evidence/comments-provider-runtime-order-smoke.json`: read/write policy order, idempotency through canonical write policy, owner `CommentsService` invocation, typed error mapping and fallback/degraded parity; status remains `in_progress` until live provider/consumer execution;
  - `rustok-comments-admin` now has explicit `admin/src/core.rs`, `admin/src/transport/mod.rs`, `admin/src/transport/native_server_adapter.rs` and `admin/src/ui/leptos.rs`; `admin/src/lib.rs` no longer contains render/business logic, does not wire pre-FFA `api.rs` and only publishes `CommentsAdmin`;
  - covered admin UI no longer calls raw `api::*` directly from Leptos render layer, but goes through module-owned transport facade;
  - status filter parsing, thread list/detail target/status labels, comment row identity/locale/body mapping and transport request/command DTO construction are moved to Leptos-free core and covered by unit tests;
  - selected-thread and locale route/query key ownership, normalization and host write intent now live in Leptos-free core on shared `UiRouteQueryIntent`, while the Leptos adapter only applies the prepared intent through the host writer;
  - fast boundary guardrail `scripts/verify/verify-comments-admin-boundary.mjs` is included in aggregate `verify:ffa:ui:migration` and locks the native-only comments admin exception without package-local GraphQL selected path; fixture suite `scripts/verify/verify-comments-admin-boundary.test.mjs` is included in aggregate `test:verify:ffa:ui:migration` and checks canonical split, legacy `api.rs`, Leptos-free core, route/query ownership, transport facade isolation, GraphQL selected-path prohibition and server-function adapter placement;
  - current admin transport remains native-only single-adapter server-function path, path is locked with typed `CommentsAdminTransportPath`/`ACTIVE_TRANSPORT_PATH`, consumes host-provided `HostRuntimeContext` instead of Loco `AppContext`, and a separate GraphQL/REST secondary path is not added as a module-documented exception without legacy admin transport surface;
  - Loco-free native admin transport evidence: `admin/src/transport/native_server_adapter.rs` uses `HostRuntimeContext`, `admin/Cargo.toml` no longer declares `loco-rs`, and `scripts/verify/verify-comments-admin-boundary.mjs` plus `scripts/verify/verify-api-surface-contract.mjs` guard the boundary;
  - FBA provider registry `crates/rustok-comments/contracts/comments-fba-registry.json`, neutral `CommentsThreadPort`/`comments.thread.v1` and static contract evidence `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json` are locked for blog and future commentable-surface consumers; write operations require shared `PortCallPolicy::write()`, read operations require shared `PortCallPolicy::read()`, and typed `PortError` mapping remains owned by `rustok-comments`;
  - fast FBA guardrail `scripts/verify/verify-comments-fba.mjs` / `npm run verify:comments:fba` checks manifest metadata, port source markers, static evidence drift and central readiness-board sync; status remains below `boundary_ready` until runtime contract execution and fallback smoke evidence land;
- Owner: `rustok-comments` module team

## Scope of work

- keep `rustok-comments` as a separate storage/domain boundary for generic comments outside `rustok-forum`;
- evolve moderation/status contract, module-owned admin UI and opt-in integrations without returning comments to the shared `content` model;
- synchronize runtime contract, local docs and host wiring as new commentable surfaces appear.

## Current state

- `rustok-comments` is already a live storage-owner for generic comments;
- `rustok-blog` uses the module in production read/write path;
- `rustok-comments-admin` is published as module-owned moderation UI;
- `rustok-comments-admin` native transport reads DB access through `rustok_api::HostRuntimeContext` and does not depend on `loco-rs`;
- observability baseline and thread status contract are already locked in runtime.

## Stages

### Stage 1. Module foundation

- [x] add crate, `CommentsModule`, permissions and module manifest;
- [x] connect module in workspace, `modules.toml`, server feature wiring and central docs;
- [x] lock local storage/API strategy inside module docs.

### Stage 2. Storage boundary

- [x] design tables `comment_threads`, `comments`, `comment_bodies`;
- [x] add module-owned migrations;
- [x] introduce entities/repositories and base `CommentService`.

### Target schema

- `comment_threads`
  - thread ownership per `(tenant_id, target_type, target_id)`
  - typed `status`, `comment_count`, `last_commented_at`
- `comments`
  - typed `thread_id`, `author_id`, `parent_comment_id`, `status`, `position`
  - no reuse of forum reply storage
- `comment_bodies`
  - locale-aware body storage with explicit `body_format`
  - canonical support for shared rich-text contracts from `rustok-content`

### Required indexes and constraints

- unique `(tenant_id, target_type, target_id)` on `comment_threads`
- unique `(comment_id, locale)` on `comment_bodies`
- ordered list indexes on `(thread_id, position)` and `(thread_id, created_at)`

### Stage 3. Domain contracts

- [x] define target binding contract for blog and generic opt-in non-forum surfaces;
- [x] define moderation/status contract for comment-domain;
- [x] reduce comment body to shared rich-text contract.

### Stage 4. Integrations

- [x] move `rustok-blog` to `rustok-comments`;
- [x] define integration of `rustok-pages` with `rustok-comments`: default integration is not
  introduced, future page-like discussion surfaces are possible only as explicit opt-in;
- [x] add transport adapters in `apps/server`.

### Stage 5. Orchestration compatibility

- [x] implement mapping between `blog comments` and `forum replies` through `rustok-content`;
- [x] cover conversion flows with end-to-end tests after orchestration service appears.

### Stage 6. Observability baseline

- [x] add module-level entrypoint/error metrics for service entry-points;
- [x] add read-path budget/query metrics for `list_comments_for_target`;
- [x] define moderation/status alerts and operator playbook after final
  comment-moderation contract is locked.

## Verification

- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- `node scripts/verify/verify-comments-admin-boundary.mjs`
- targeted tests for moderation/status contract, blog integration and admin UI runtime wiring

## Update rules

1. When changing comment-domain contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata and UI wiring, synchronize `rustok-module.toml`.

## Current state details

- `rustok-comments` — no longer a scaffold, but a live storage-owner for generic comments;
- `rustok-blog` already uses the module in production read/write path;
- `rustok-pages` does not get a default comments surface; pages-level integration is consciously
  left outside the current product scope;
- observability baseline for service-layer is already in place: module entrypoint/error
  counters, span duration/error and read-path budget/query metrics on list path;
- thread status contract is already enforced in runtime: `closed` blocks new
  create-path, while `spam|trash` require moderation scope;
- further module scope is now related not to split, but to expanding moderation and
  product-level integrations.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
