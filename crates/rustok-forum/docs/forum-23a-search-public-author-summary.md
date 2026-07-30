# FORUM-23A: privacy-aware Search author summaries

Date: 2026-07-30

Status: `source_complete_execution_pending`

Latest slice: `FORUM-23A6`

This slice advances the canonical `FORUM-23` visibility-aware Search track. Public Forum topic
and approved-reply documents obtain author presentation from the Profiles owner instead of
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
`ProfileUpdated` for handle, display content, locale, visibility, media, and self-service upsert
changes. Search treats that owner event as a Forum projection event whenever the Forum source is
composed.

The privacy-critical `update_my_profile_visibility` path writes the new visibility and the
`ProfileUpdated` outbox envelope in the same Profiles-owned database transaction. If outbox
publication fails, the visibility write is explicitly rolled back and the mutation returns a
retryable owner error.

`FORUM-23A2` applies the same owner rule to `update_my_profile_handle`. Handle normalization,
duplicate-owner validation, the profile row update, and the `ProfileUpdated` outbox envelope share
one transaction. If event publication fails, the handle write is rolled back.

`FORUM-23A3` applies the same rule to `update_my_profile_content`. Display-name normalization, the
profile revision timestamp, the selected localized display-name/biography row, and the durable
`ProfileUpdated` envelope commit together. The shared publisher accepts the actual Profiles owner
model returned by SeaORM, matching the handle and visibility helpers.

`FORUM-23A4` applies the owner rule to `update_my_profile_media`. Media ownership and access are
validated before opening the Profiles transaction. The avatar/banner identifiers, profile revision
timestamp, and durable `ProfileUpdated` envelope then commit in one transaction. If outbox
publication fails, the media owner write is rolled back. A new public avatar therefore cannot
commit while Forum Search retains the previous `avatar_media_id` because its invalidation was lost.
Banner remains owner data and is never serialized into Forum Search.

`FORUM-23A5` applies the same rule to `update_my_profile_locale`. The preferred locale is normalized
before the owner transaction. The tenant-scoped profile row, revision timestamp, and durable
`ProfileUpdated` envelope then commit together. If publication fails, the locale write is rolled
back. The path preserves the existing selection-only rule: changing preferred locale does not copy,
insert, or update localized display content.

`FORUM-23A6` applies the owner rule to `upsert_my_profile`. Media ownership is validated before the
transaction. Inside one Profiles-owned transaction the path checks tenant-scoped handle ownership,
creates or updates the profile row, upserts the selected localized display-name/biography row,
replaces taxonomy-backed profile tags, and writes the durable `ProfileUpdated` envelope. Event
failure rolls back every owner row, including newly created profiles, translations, and tag
relations. The GraphQL mutation no longer has a post-commit event publisher.

All self-service mutations that emit `ProfileUpdated` now use the shared transactional publisher in
`crates/rustok-profiles/src/profile_updated_event.rs`, so their event envelope and retryable error
classification cannot drift independently. This does not claim full owner-write coverage: the
Profiles CLI/backfill path still calls the service upsert without publishing `ProfileUpdated` and
requires a separate invalidation or rebuild proof.

The event is stored under `forum_author:<user_id>`. This scope is intentionally a redaction
barrier: it is not stale-skipped against the unrelated full Forum wall-clock watermark. The
consumer rebuilds the Forum tenant projection from current owner state, so committed visibility,
handle, public display-name, locale selection, avatar, and self-service upsert changes replace stale
Search presentation.

The existing tenant advisory lock, durable retry, dead-letter bound, and periodic/opportunistic
inbox reconciliation remain unchanged. This slice does not claim that general Forum producer
ordering is solved; owner-issued monotonic revisions remain an ordering-hardening task. It also
does not treat `UserDeleted` as sufficient deletion evidence because that event does not prove
that the Profiles owner has already removed or hidden its state.

## Search shape

Topic and approved-reply documents use:

- `payload.author` for the bounded public summary or `null`;
- `facets.author_id` with a non-null value only when the Profiles owner returned a public summary;
- `facets.has_public_author` as an explicit filterable boolean;
- the Search `handle` column for the permitted public handle.

The previous raw `payload.author_id` value is removed. Category documents and the public Forum
discovery contract are unchanged.

## Compatibility

No database migration, Search query API change, Forum GraphQL/REST change, Forum owner-storage
change, Profiles owner-storage change, dependency change, or `Cargo.lock` change is introduced.
Existing Search document rows are replaced by the next Forum rebuild or relevant durable event.

## Remaining FORUM-23 scope

- owner-issued monotonic projection revisions across Forum producers;
- an owner-ordered profile or account deletion invalidation contract;
- durable Search invalidation for CLI/backfill profile creation, or retained evidence that the
  responsible rebuild always follows it;
- bounded category-subtree, tag, locale, date, solved, kind, channel/group, attachment, and
  remaining author filters;
- member Search projections;
- maintainer-executed PostgreSQL redaction, rebuild, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
cargo check -p rustok-profiles --all-targets
cargo test -p rustok-profiles error -- --nocapture
cargo test -p rustok-forum search_projection_author -- --nocapture
cargo test -p rustok-search forum_inbox -- --nocapture
cargo test -p rustok-search ingestion -- --nocapture
node scripts/verify/verify-forum-search-public-author-summary.mjs
node scripts/verify/verify-forum-search-projection.mjs
cargo xtask module validate profiles
cargo xtask module validate forum
```
