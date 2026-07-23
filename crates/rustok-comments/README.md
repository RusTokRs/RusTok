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
- Own comment position allocation and thread counters below transport/service callers. Transactional comment inserts lock the tenant thread row before deriving the next position; explicit transactional counter refreshes lock the same row and count active owner rows instead of trusting a stale caller value.
- Keep status-only and metadata-only thread updates out of the counter write path.
- Repair historical counters and positions through append-only PostgreSQL/SQLite migration and enforce unique `(thread_id, position)` storage.
- Document operator-facing moderation/status alerts so `closed` thread conflicts, moderation drift, and DB incidents are triaged consistently.

## Thread write invariants

`comment::ActiveModelBehavior` is the position owner for transactional insert
paths. It ignores a supplied position, serializes on the tenant thread row, and
allocates the next checked position. Direct/bulk bypasses remain protected by the
unique thread-position index.

`comment_thread::ActiveModelBehavior` is activated only when an update explicitly
sets `comment_count`. The normal create/delete transaction uses that path to
recompute the counter from comments with `deleted_at IS NULL` while holding the
same owner lock. Status and metadata updates leave the counter field unchanged.

`m20260723_000008_repair_comment_thread_counters` backfills existing counts,
renumbers positions deterministically, and converts the thread-position index to
a unique index. Contract evidence lives in
`contracts/evidence/comments-thread-write-invariants.json`. The executable target
contains SQLite coverage and an env-gated two-connection PostgreSQL concurrency
case selected by `RUSTOK_COMMENTS_TEST_DATABASE_URL`; neither tests nor source
verifiers are claimed as executed.

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
