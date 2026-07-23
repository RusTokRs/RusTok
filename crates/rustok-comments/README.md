# rustok-comments

## Purpose

`rustok-comments` owns the generic comments domain for RusToK.

## Responsibilities

- Provide a dedicated storage boundary for classic comments outside the forum domain.
- Serve as the canonical storage owner for blog comments and other opt-in classic non-forum comments.
- Keep `comments` separate from forum topics and forum replies.
- Expose module metadata, permissions, and future migrations for the comments domain.
- Publish the module-owned Leptos admin moderation UI crate `rustok-comments-admin`.
- Accept only `rustok-api::RichTextDocument` for comment writes, enforce the
  server-selected `rustok-content::richtext` `comment` profile, and return
  `RichTextView` plus server-derived plain text.
- Reuse shared locale fallback semantics from `rustok-content` so comment reads match other localized content modules.
- Emit module-level entrypoint/error metrics and bounded read-path telemetry for the comments service surface.
- Enforce thread and moderation status rules in the service layer instead of treating them as storage-only fields.
- Document operator-facing moderation/status alerts so `closed` thread conflicts, moderation drift, and DB incidents are triaged consistently.

## Interactions

- Depends on `rustok-core` for module contracts and permission vocabulary.
- Depends on `rustok-content` for shared rich-text and locale-resolution helpers.
- Stores canonical ProseMirror/Tiptap root JSON in owner-local localized body
  rows; locale remains row context and there is no content-format selector.
- Integrates with `rustok-blog` today.
- May back future opt-in non-forum discussion surfaces, but `rustok-pages` is not a default integration target.
- Must not become the storage backend for `rustok-forum`.
- `rustok-comments-admin` uses native Leptos `#[server]` functions directly over `CommentsService`;
  there is no GraphQL/REST fallback because the comments domain did not have a legacy transport surface
  of its own.
- `rustok-comments-admin` receives native DB access from `rustok_api::HostRuntimeContext`, not a host-wide `AppContext`.
- `rustok-comments-admin` keeps selected-thread and locale route-query normalization/write policy in
  its framework-agnostic core using shared `UiRouteQueryUpdate`, while the Leptos adapter only
  applies the prepared host updates.

## Entry points

- `CommentsModule`
- `CommentsService`
- `rustok-comments-admin`

See also `docs/README.md`.
