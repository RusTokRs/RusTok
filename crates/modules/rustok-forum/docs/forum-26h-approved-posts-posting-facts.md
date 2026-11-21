# forum-approved-posts-index-debt

Status: source-ready / unvalidated; source-ready / maintainer execution pending.

This owner note records the source boundary required by the static contract. Runtime execution, compiled verification and admission remain maintainer-owned; no runtime result is claimed here.

## Source contract

- ## Performance boundary
- does not add dedicated exact-author count indexes
- query plans against representative tenant cardinality
- author-count index and query-plan hardening
- before any posting owner begins invoking the policy composer synchronously

Tests, Cargo, formatting, and CI were not run by the implementation agent; this note records source review only.
- # FORUM-26H approved-posts posting facts
- current `forum_topics` and `forum_replies`
- one aggregate statement
- immediately `open`
- `open`, `closed`, and `archived`
- status is `approved`
- parent topic is not soft-deleted
- authoritative `0`
- Storage execution failures return retryable `Unavailable`
- PostgreSQL and SQLite
- requires the exact user actor
- does not expose topic IDs
- `forum_user_stats`
- registers four unique owner providers
- no shared distributed rate-limit reservation
- next bounded FORUM-26 slice
