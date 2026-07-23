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

The shared `rustok-api::richtext` document contract and
`rustok-content::richtext` `comment` profile are implemented and Comments is
the first owner cut over to them. `CreateCommentInput` and
`UpdateCommentInput` accept only `RichTextDocument`; `CommentRecord` returns
`RichTextView` plus the server-derived plain-text projection. Comment body rows
persist canonical ProseMirror/Tiptap JSON without a format selector, and public
previews use the plain-text projection. The cutover migration fails closed on
pre-existing non-canonical rows so an offline conversion can be completed
before the schema column is dropped.

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

3. **Extend moderation and opt-in integrations through comment ownership.**
   Add a new commentable surface only with explicit target binding, moderation,
   rich-text, tenant, and observability contracts; do not reuse forum storage.
   **Depends on:** the consuming module's product requirement and public API.
   **Done when:** the new surface has owner-owned storage and transport tests,
   and its opt-in decision is documented.

4. **Keep operational guidance synchronized with thread semantics.** Update
   status alerts, moderation playbook, metrics, and local docs with a change to
   thread lifecycle or comment delivery.
   **Depends on:** the changed comments runtime contract.
   **Done when:** closed/spam/trash behavior and recovery are observable and
   documented for operators.

5. **Close the direct-write richtext bypass and join the atomic cutover.**
   **Implemented for Comments.** A direct `CommentsThreadPort` or service
   write accepts the typed `RichTextDocument`, selects the `comment` profile
   server-side, and passes the strict validator. `comment_bodies` no longer
   stores a format selector; reads use canonical HTML/plain-text projections.
   The remaining verification is runtime evidence for every consumer and the
   offline conversion procedure for existing Markdown rows.
   **Depends on:** the
   [central Richtext plan](../../../docs/modules/rich-text-implementation-plan.md)
   and synchronized Blog consumer contract.
   **Done when:** invalid/empty/oversized documents fail at every entry point,
   no direct port bypass exists, and Next/Leptos reads share the server renderer.

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
- Targeted moderation/status, blog integration, comment-port, and admin runtime
  tests.
- `cargo check -p rustok-comments`
- `cargo check -p rustok-blog`

## Change rules

1. Keep generic comment storage and moderation in this module.
2. Preserve tenant-thread locking for transactional position and explicit counter
   derivation; no caller or transport may source `position` or `comment_count`.
3. Status-only and metadata-only thread updates must not set `comment_count`.
4. Preserve the identity-lock before first-thread insert and keep its unique
   `(tenant_id, target_type, target_id)` key.
5. Keep migrations append-only and preserve both database uniqueness invariants.
6. Update local/central contracts when the Comments boundary changes.
