# rustok-comments

## Purpose

`rustok-comments` owns the generic comments domain for RusToK.

## Responsibilities

- Provide a dedicated storage boundary for classic comments outside the forum domain.
- Serve as the canonical storage owner for blog comments and other opt-in classic non-forum comments.
- Keep `comments` separate from forum topics and forum replies.
- Expose module metadata, permissions, and future migrations for the comments domain.
- Publish the module-owned Leptos admin moderation UI crate `rustok-comments-admin`.
- Align comment-body contracts with shared rich-text rules from `rustok-content`.
- Reuse shared locale fallback semantics from `rustok-content` so comment reads match other localized content modules.
- Emit module-level entrypoint/error metrics and bounded read-path telemetry for the comments service surface.
- Enforce thread and moderation status rules in the service layer instead of treating them as storage-only fields.
- Document operator-facing moderation/status alerts so `closed` thread conflicts, moderation drift, and DB incidents are triaged consistently.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-content` for shared rich-text and locale-resolution helpers.
- Integrates with `rustok-blog` today.
- May back future opt-in non-forum discussion surfaces, but `rustok-pages` is not a default integration target.
- Must not become the storage backend for `rustok-forum`.
- `rustok-comments-admin` uses native Leptos `#[server]` functions directly over `CommentsService`;
  there is no GraphQL/REST fallback because the comments domain did not have a legacy transport surface
  of its own.

## Entry points

- `CommentsModule`
- `CommentsService`
- `rustok-comments-admin`

See also `docs/README.md`.
