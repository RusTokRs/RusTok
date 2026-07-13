# Implementation Plan for `rustok-forum`

## Current state

`rustok-forum` owns categories, topics, replies, moderation, persistence, and
its GraphQL, REST, admin, and storefront surfaces. The admin and storefront
packages use module-owned core, transport, and Leptos adapter layers; the fast
boundary checks keep the removed legacy `api.rs` facades from returning.

Forum declares the Page Builder consumer contract in `rustok-module.toml`.
The widget catalog, compatibility metadata, fallback profiles, and static
fallback matrix are source-locked. They are not proof that a forum tenant has
run the provider in production: the current Wave packet must be replaced by
an observed control-plane run after the `pages` reference consumer is ready.

## FORUM-00 through FORUM-08 audit checkpoint

The runtime and schema work merged through FORUM-08 establishes PostgreSQL and
SQLite regression profiles, tenant-composite relation constraints, typed topic
and reply statuses, atomic category translation writes, category hierarchy
cycle guards, serialized counter mutations, publication-aware moderation,
unique reply positions, soft-delete tombstones, and revision history.

The 2026-07-13 post-merge audit found three database invariants that required
additional hardening:

- PostgreSQL reply positions must be allocated from a monotonic per-topic
  counter rather than recalculated with `MAX(position) + 1` for every insert.
- Physical category deletion must be rejected while the category owns topics or
  child categories, so category cascades cannot erase discussion and revision
  history accidentally.
- Revision locale columns must use the platform `VARCHAR(32)` locale width.

Migration `m20260713_000010_harden_forum_wave_invariants` applies those fixes.
`tests/wave_invariants_postgres.rs` proves the monotonic allocator and category
delete guard, while `tests/runtime_regression_baseline.rs` verifies the schema,
trigger, and locale-width contract.

FORUM-08B moves the public topic/reply lifecycle path into owner services:

- `ReplyService` rejects locked topics with a typed error, allocates reply
  positions, and publishes counters/events only for approved replies.
- `ReplyService::delete` atomically claims the live reply, explicitly redacts
  content, persists the tombstone, removes solution state, updates projections,
  and emits the status event in one transaction.
- `TopicService::delete` atomically claims the live topic, explicitly redacts
  the whole thread, preserves revisions, persists topic/reply tombstones,
  removes solution and flex state, and updates category/user projections in one
  transaction.
- database triggers remain enabled as invariant protection for direct SQL and
  compatibility callers; they are no longer the primary root-service workflow.

`tests/owner_lifecycle_sqlite.rs` exercises locked-topic rejection, moderated
reply publication, reply tombstones, topic tombstones, projection updates, and
revision preservation through the public owner services.

The audit does not declare the complete product plan finished. Follow-up work
must still expose a real category tree read model, replace unbounded page sizes,
publish the complete owner event catalog, and retire direct use of the raw
compatibility service modules after downstream callers have migrated.

## FFA/FBA status

- FFA status: `in_progress` — module-owned admin and storefront FFA surfaces
  exist; continued changes must retain the core/transport/UI boundary.
- FBA status: `boundary_ready` — the Forum/Page Builder consumer contract and
  fallback matrix exist, while live provider-consumer runtime evidence is not
  yet accepted for rollout.
- Structural shape: `core_transport_ui`
- Evidence: `scripts/verify/verify-forum-admin-boundary.mjs`,
  `scripts/verify/verify-forum-storefront-boundary.mjs`,
  `contracts/evidence/fw2-fallback-static-matrix.json`, and
  `contracts/evidence/forum-wave1-rollout-evidence.json`.

## Open results

1. Replace the Wave 1 packet with an observed forum tenant control-plane run
   after `rustok-pages` has passed the Page Builder reference-consumer gate.
   Done when the packet correlates builder write, forum publish, and storefront
   read for every required fallback profile without a waiver.
   Dependency: Page Builder provider readiness and the verified `pages`
   integration. Verification: `npm run verify:page-builder:consumer:forum` and
   `npm run verify:forum:wave-evidence-freshness`.
2. Implement the forum widget consumer only through the public Page Builder
   capability contract. Done when topic-list, topic-detail, and reply-stream
   widgets preserve the declared `readonly`, `degraded`, and `hidden`
   fallbacks without importing Page Builder internals.
   Dependency: the provider persistence/rendering endpoints selected in the
   Page Builder plan. Verification: the consumer readiness verifier plus
   targeted forum widget contract tests.
3. Preserve forum ownership while evolving the admin and storefront products.
   Done when each changed route uses the module transport facade, applies forum
   visibility/moderation policy, and leaves the legacy facades absent.
   Dependency: host composition only. Verification:
   `npm run verify:forum:admin-boundary` and
   `npm run verify:forum:storefront-boundary`.
4. Retire direct imports of `services::topic::TopicService` and
   `services::reply::ReplyService` after downstream compatibility callers use
   the root owner services. Done when the raw modules can become crate-private
   without breaking workspace builds or external module contracts.

## Verification

- `cargo test -p rustok-forum --test runtime_regression_baseline`
- `cargo test -p rustok-forum --test wave_invariants_postgres`
- `cargo test -p rustok-forum --test soft_delete_revision_postgres`
- `cargo test -p rustok-forum --test soft_delete_revision_sqlite`
- `cargo test -p rustok-forum --test owner_lifecycle_sqlite`
- `npm run verify:forum:admin-boundary`
- `npm run verify:forum:storefront-boundary`
- `npm run verify:page-builder:consumer:forum`
- `npm run verify:forum:wave-evidence-freshness`

## Boundaries

- `rustok-forum` owns forum domain policy, widget data contracts, and consumer
  fallback behaviour.
- `rustok-page-builder` owns GrapesJS capability delivery, feature flags,
  provider persistence/rendering, and rollout control-plane mechanics.
- Hosts compose the owner-owned forum surfaces and do not absorb forum policy
  or Page Builder provider internals.
