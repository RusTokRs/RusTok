# rustok-comments

## Purpose

`rustok-comments` owns the generic comments domain for RusToK.

## Responsibilities

- Provide a dedicated storage boundary for classic comments outside the forum domain.
- Serve as the canonical storage owner for Blog comments and other opt-in classic non-forum comments.
- Keep `comments` separate from forum topics and forum replies.
- Expose module metadata, permissions, migrations, and the `CommentsThreadPort` provider boundary.
- Publish the module-owned Leptos admin moderation UI crate `rustok-comments-admin`.
- Align comment-body contracts with shared rich-text rules from `rustok-content`.
- Reuse shared locale fallback semantics from `rustok-content` so comment reads match other localized content modules.
- Emit module-level entrypoint/error metrics and bounded read-path telemetry for the comments service surface.
- Enforce thread and moderation status rules in the service layer instead of treating them as storage-only fields.
- Own transactional comment position allocation and explicit thread counter refreshes below transport/service callers.
- Serialize first-thread creation by tenant/target identity without transport retries.
- Keep status-only and metadata-only thread updates out of the counter write path.
- Repair historical counters and positions through append-only PostgreSQL/SQLite migrations and enforce unique `(thread_id, position)` storage.

## Thread write invariants

`comment::ActiveModelBehavior` is the position owner for transactional insert
paths. It ignores a supplied position, serializes on the tenant thread row, and
allocates the next checked position. Direct/bulk bypasses remain protected by the
unique thread-position index.

`comment_thread::ActiveModelBehavior` has two owner responsibilities:

- on insert, it upserts and locks a persistent
  `comment_thread_identity_locks` row keyed by
  `(tenant_id, target_type, target_id)`, then checks for an existing canonical
  thread before the SQL INSERT;
- on an update that explicitly sets `comment_count`, it locks the thread row and
  recomputes the counter from comments with `deleted_at IS NULL`.

A concurrent first-thread creator therefore returns through the existing service
lookup fallback without aborting its PostgreSQL transaction on a unique violation.
Status and metadata updates leave the counter field unchanged.

`m20260723_000008_repair_comment_thread_counters` backfills counts, renumbers
positions, and creates the unique thread-position index.
`m20260723_000009_add_comment_thread_identity_locks` creates the persistent unique
identity-lock registry. Contract evidence lives in
`contracts/evidence/comments-thread-write-invariants.json`.

Executable targets contain SQLite coverage, a two-connection PostgreSQL
create/delete harness, and a separate two-service first-thread harness selected by
`RUSTOK_COMMENTS_TEST_DATABASE_URL`. Tests and source verifiers are written but
are not claimed as executed.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-content` for shared rich-text and locale-resolution helpers.
- Integrates with `rustok-blog` today.
- May back future opt-in non-forum discussion surfaces, but `rustok-pages` is not a default integration target.
- Must not become the storage backend for `rustok-forum`.
- `rustok-comments-admin` uses native Leptos `#[server]` functions directly over `CommentsService`; there is no GraphQL/REST fallback because the comments domain did not have a legacy transport surface of its own.
- `rustok-comments-admin` receives native DB access from `rustok_api::HostRuntimeContext`, not a host-wide `AppContext`.
- `rustok-comments-admin` keeps selected-thread and locale route-query normalization/write policy in its framework-agnostic core using shared `UiRouteQueryUpdate`, while the Leptos adapter only applies the prepared host updates.

## Entry points

- `CommentsModule`
- `CommentsService`
- `CommentsThreadPort`
- `rustok-comments-admin`

See also `docs/README.md`.
