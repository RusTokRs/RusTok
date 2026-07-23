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
comment inserts lock the tenant thread row before assigning the next position.
Explicit transactional counter refreshes take the same owner lock and replace a
caller-supplied `comment_count` with the exact number of tenant comments whose
`deleted_at` is null. Status-only and metadata-only thread updates do not activate
a counter write.

First-thread creation has a separate owner identity lock. Before a transactional
thread insert, `comment_thread::ActiveModelBehavior` upserts a persistent
`comment_thread_identity_locks` row with `ON CONFLICT DO NOTHING`, locks that row,
and checks for the canonical thread. A concurrent creator receives an intentional
application `DbErr` before its SQL INSERT, leaving the PostgreSQL transaction
usable by the existing `find_or_create_thread_in_tx` fallback. This prevents the
old unique-violation/aborted-transaction failure mode without transport retries.

Append-only migrations repair historical counters, deterministically renumber
historical positions, enforce `UNIQUE(thread_id, position)`, and create the unique
identity-lock key `(tenant_id, target_type, target_id)` for PostgreSQL and SQLite.

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
  with status `executable_no_run`.
- Executable targets:
  `crates/rustok-comments/tests/thread_write_invariants.rs` and
  `crates/rustok-comments/tests/thread_creation_concurrency.rs`.
- Both PostgreSQL targets use two independent one-connection pools, an isolated
  schema, and `RUSTOK_COMMENTS_TEST_DATABASE_URL` or PostgreSQL `DATABASE_URL`.
- `scripts/verify/verify-comments-thread-write-invariants.mjs`, its negative
  fixtures, and `npm run verify:comments:fba` lock transactional position/count
  behavior, status-only preservation, repair migrations, identity-lock storage,
  service fallback, and both concurrency harnesses.
- Public-port create/delete publish `comment.created` and `comment.deleted`
  through `TransactionalEventBus::publish_in_tx`. Blog's idempotent reply-count
  projection is implemented statically under
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
6. Added PostgreSQL/SQLite repair for stale counters and duplicate positions, then
   enforced `UNIQUE(thread_id, position)`.
7. Added persistent identity-lock storage and transactional first-thread
   serialization around the existing service fallback.
8. Added SQLite invariant coverage and a PostgreSQL create/create followed by
   create/soft-delete harness for an existing thread.
9. Added a separate PostgreSQL `thread_creation_concurrency` target proving that
   two `CommentsService` instances creating the first comments for one target
   return one thread with positions `1/2` and count `2`.
10. Added machine-readable evidence, current-only source/negative verifiers, and
    integrated all thread invariants into the main Comments FBA gate. These
    targets are written but not executed.

## Open results

1. **Execute thread concurrency evidence.** Run both env-gated PostgreSQL targets
   and retain active-row counts, unique/gap-free positions, status-only
   preservation, one-thread first-create evidence, and migration output.
   **Done when:** runtime evidence confirms the owner locks under real concurrent
   PostgreSQL transactions.

2. **Narrow the service fallback error class.** `find_or_create_thread_in_tx`
   currently retries canonical lookup for every thread insert error. Preserve the
   identity-conflict fallback while propagating unrelated infrastructure errors.
   **Done when:** typed storage errors cannot be converted into a misleading
   `CommentThreadNotFound`.

3. **Implement and execute the Blog reply-count event projection.** Consume
   `comment.created` and `comment.deleted` idempotently, publish the Blog-owned
   update event in the projection transaction, and prove retry/degraded behavior.

4. **Execute CommentsThreadPort runtime and consumer evidence.** Cover read/write
   policy, idempotency, typed errors, fallback profiles, and Blog compatibility.

5. **Close the direct-write rich-text bypass.** Migrate direct comment writes to
   typed `RichTextDocument`, server-selected `comment` profile, and canonical
   HTML/plain-text projections. See
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md).

## Verification

- `cargo test -p rustok-comments --test thread_write_invariants`
- `RUSTOK_COMMENTS_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-comments --test thread_write_invariants postgres_concurrent_creates_and_delete_preserve_thread_invariants`
- `RUSTOK_COMMENTS_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-comments --test thread_creation_concurrency`
- `node scripts/verify/verify-comments-thread-write-invariants.mjs`
- `node scripts/verify/verify-comments-thread-write-invariants.test.mjs`
- `npm run verify:comments:admin-boundary`
- `npm run verify:comments:fba`
- `cargo xtask module validate comments`
- `cargo xtask module test comments`

## Change rules

1. Keep generic comment storage and moderation in this module.
2. Preserve tenant-thread locking for transactional position and explicit counter
   derivation; no caller or transport may source `position` or `comment_count`.
3. Status-only and metadata-only thread updates must not set `comment_count`.
4. Preserve the identity-lock before first-thread insert and keep its unique
   `(tenant_id, target_type, target_id)` key.
5. Keep migrations append-only and preserve both database uniqueness invariants.
6. Update local/central contracts when the Comments boundary changes.
