# Implementation plan for `rustok-comments`

## Current state

`rustok-comments` owns generic comment threads, comments, localized bodies,
thread status/moderation, and comment-domain observability. It is separate from
forum replies and shared content storage. Blog uses the module on its production
read/write path; page-like surfaces require explicit opt-in.

The admin moderation surface is an intentional native-only exception: it has a
module-owned core, native transport facade, and Leptos adapter backed by
`HostRuntimeContext`. Thread and locale route/query policy is core-owned, and UI
does not call raw transport.

Thread write consistency is owner-enforced below the service facade. Transactional
comment inserts use `ActiveModelBehavior` to lock the tenant thread row before
assigning the next position, so caller-supplied or stale positions are ignored.
Explicit transactional counter refreshes take the same owner lock and replace a
caller-supplied `comment_count` with the exact number of tenant comments whose
`deleted_at` is null. Status-only and metadata-only thread updates do not activate
a counter write. An append-only repair migration recalculates historical counts,
deterministically renumbers historical positions, and promotes the existing
position index to `UNIQUE(thread_id, position)` for PostgreSQL and SQLite.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `CommentsThreadPort` / `comments.thread.v1` in
  `crates/rustok-comments/contracts/comments-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json`
  and `crates/rustok-comments/contracts/evidence/comments-provider-runtime-order-smoke.json`.
- Thread write invariant evidence:
  `crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json`
  with status `executable_no_run` and test target
  `crates/rustok-comments/tests/thread_write_invariants.rs`.
- The test target contains SQLite owner-policy coverage plus an env-gated,
  two-connection PostgreSQL harness selected by
  `RUSTOK_COMMENTS_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- `scripts/verify/verify-comments-admin-boundary.mjs`,
  `scripts/verify/verify-comments-thread-write-invariants.mjs`, and
  `npm run verify:comments:fba` lock the native-only admin boundary, provider
  policy order, transactional position allocation, exact counters, status-only
  preservation, repair migration, database uniqueness fallback, and PostgreSQL
  concurrency target.
- Public-port create/delete publish `comment.created` and `comment.deleted`
  through `TransactionalEventBus::publish_in_tx` when the provider is runtime
  composed with its owner event bus. Blog's idempotent reply-count projection is
  implemented statically under
  `DECISIONS/2026-07-16-comments-blog-event-projection.md`; runtime delivery,
  retry, and recovery evidence remain open.

## Completed implementation slices

1. Added the transport-neutral `CommentsThreadPort` provider boundary and shared
   read/write `PortCallPolicy` enforcement.
2. Added approved-only public reads, native admin moderation, localized body
   fallback, and transactional create/delete event publication.
3. Replaced unprotected `MAX(position) + 1` allocation with a tenant-thread owner
   lock in comment `ActiveModelBehavior` for transactional inserts.
4. Replaced stale read-modify-write thread counters with an exact active-row count
   under the same tenant-thread owner lock for explicit counter refreshes.
5. Prevented status-only or metadata-only thread updates from becoming counter
   writers.
6. Added an append-only PostgreSQL/SQLite repair migration for stale counters and
   duplicate historical positions, then enforced `UNIQUE(thread_id, position)`.
7. Added executable SQLite invariant coverage and an env-gated PostgreSQL target
   with two independent connections. The PostgreSQL target runs concurrent
   create/create followed by concurrent create/soft-delete and verifies positions
   `1..3` plus the final exact active count.
8. Added machine-readable evidence, current-only source/negative verifiers, and
   integrated the invariant contract into the main Comments FBA gate. These
   targets are written but not executed.

## Open results

1. **Execute thread concurrency evidence.** Run the SQLite and env-gated
   PostgreSQL cases and retain final active-row count, unique/gap-free positions,
   status-only preservation, and historical repair output.
   **Done when:** database evidence confirms the owner hooks and unique index under
   real concurrent PostgreSQL transactions.

2. **Implement and execute the Blog reply-count event projection.** Consume
   `comment.created` and `comment.deleted` idempotently, publish the Blog-owned
   update event in the projection transaction, and prove retry/degraded behavior.
   **Depends on:** runtime event delivery and projection storage fixtures.

3. **Execute CommentsThreadPort runtime and consumer evidence.** Cover read and
   write paths, canonical read/write policy, idempotency, typed errors,
   fallback/degraded profiles, and Blog embedded/native compatibility before FBA
   promotion.
   **Done when:** evidence proves provider and consumer behavior without a direct
   comments-service bypass.

4. **Extend moderation and opt-in integrations through comment ownership.** Add a
   new commentable surface only with explicit target binding, moderation,
   rich-text, tenant, and observability contracts; do not reuse forum storage.

5. **Close the direct-write rich-text bypass and join the atomic cutover.** A
   direct `CommentsThreadPort` or service write must accept the typed
   `RichTextDocument`, select the `comment` profile server-side, and pass the same
   strict validator as Blog-integrated writes. Migrate `comment_bodies`, remove
   client-selectable formats and body/`content_json` duplication, and use the
   canonical HTML/plain-text projections for moderation, storefront, events, and
   Search/Index consumers. See the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md).

## Verification

- `cargo test -p rustok-comments --test thread_write_invariants`
- `RUSTOK_COMMENTS_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-comments --test thread_write_invariants postgres_concurrent_creates_and_delete_preserve_thread_invariants`
- `node scripts/verify/verify-comments-thread-write-invariants.mjs`
- `node scripts/verify/verify-comments-thread-write-invariants.test.mjs`
- `npm run verify:comments:admin-boundary`
- `npm run verify:comments:fba`
- `cargo xtask module validate comments`
- `cargo xtask module test comments`
- Targeted moderation/status, Blog integration, comment-port, and admin runtime
  tests.

## Change rules

1. Keep generic comment storage and moderation in this module.
2. Preserve tenant-thread locking for transactional position and explicit counter
   derivation; no caller or transport may become the source of `position` or
   `comment_count`.
3. Status-only and metadata-only thread updates must not set `comment_count`.
4. Keep the repair migration append-only and preserve the unique thread-position
   database invariant.
5. Update local docs, `rustok-module.toml`, and consumer docs with a comment
   contract or opt-in integration change.
6. Update this status block and `docs/modules/registry.md` with an FFA/FBA boundary
   change.
