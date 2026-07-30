# FORUM-23A: privacy-aware Search author summaries

Date: 2026-07-30

Status: `source_complete_execution_pending`

This slice advances the canonical `FORUM-23` visibility-aware Search track. Public Forum topic
and approved-reply documents now obtain author presentation from the Profiles owner instead of
serializing the raw Forum author identifier.

The machine-readable contract is
`crates/rustok-forum/contracts/forum-search-public-author-summary.json`.

## Owner boundary

`ForumSearchProjectionSource` calls `ProfilePresentationService` with its anonymous audience.
Profiles remains the authority for profile privacy and localized presentation; Forum does not
copy visibility rules or read profile tables directly.

When Profiles permits anonymous presentation, the Search document may contain only:

- `user_id`;
- `handle`;
- `display_name`;
- `avatar_media_id`.

The public handle also populates the Search `handle` column, and the public handle/display name
extend the document keywords. Profile tags, preferred locale, visibility state, biography,
banner, and other owner data are not copied into Forum Search payloads.

When the profile is absent or denied, the projected author is `null`, the Search handle remains
empty, and no author identifier is exposed. A Profiles owner failure is not converted into an
empty author: it fails projection so the durable Search inbox can retry.

## Durable redaction

Embedded summaries require invalidation when profile presentation changes. Profiles publishes
`ProfileUpdated` after a successful owner write for handle, display content, locale, visibility,
and media changes. Search now treats that owner event as a Forum projection event whenever the
Forum source is composed.

The event is stored under `forum_author:<user_id>`. This scope is intentionally a redaction
barrier: it is not stale-skipped against the unrelated full Forum wall-clock watermark. The
consumer rebuilds the Forum tenant projection from current owner state, so a profile changed to
private removes the previously stored public summary.

The existing tenant advisory lock, durable retry, dead-letter bound, and periodic/opportunistic
inbox reconciliation remain unchanged. This slice does not claim that general Forum producer
ordering is solved; owner-issued monotonic revisions remain the next ordering hardening task.
It also does not treat `UserDeleted` as sufficient deletion evidence because that event does not
prove that the Profiles owner has already removed or hidden its state.

## Search shape

Topic and approved-reply documents now use:

- `payload.author` for the bounded public summary or `null`;
- `facets.author_id` with a non-null value only when the Profiles owner returned a public summary;
- `facets.has_public_author` as an explicit filterable boolean;
- the Search `handle` column for the permitted public handle.

The previous raw `payload.author_id` value is removed. Category documents and the public Forum
discovery contract are unchanged.

## Compatibility

No database migration, Search query API change, Forum GraphQL/REST change, Forum owner-storage
change, Profiles owner-storage change, or Cargo dependency change is introduced. Existing Search
document rows are replaced by the next Forum rebuild or relevant durable event.

## Remaining FORUM-23 scope

- owner-issued monotonic projection revisions across Forum producers;
- an owner-ordered profile or account deletion invalidation contract;
- bounded category-subtree, tag, locale, date, solved, kind, channel/group, attachment, and
  remaining author filters;
- member Search projections;
- maintainer-executed PostgreSQL redaction, rebuild, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo test -p rustok-forum search_projection_author -- --nocapture
cargo test -p rustok-search forum_inbox -- --nocapture
cargo test -p rustok-search ingestion -- --nocapture
node scripts/verify/verify-forum-search-public-author-summary.mjs
node scripts/verify/verify-forum-search-projection.mjs
cargo xtask module validate forum
```
