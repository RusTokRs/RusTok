# FORUM-23A: privacy-aware Search author summaries

Date: 2026-07-30

Status: `source_complete_execution_pending`

Latest slice: `FORUM-23A11`

This track projects privacy-aware public author presentation into Forum topic and approved-reply
Search documents. Profiles remains the presentation authority; Forum never serializes a raw author
identifier merely because Forum owns a topic or reply row.

The machine-readable contract is
`crates/rustok-forum/contracts/forum-search-public-author-summary.json`.

## Owner boundary

`ForumSearchProjectionSource` calls `ProfilePresentationService` with its anonymous audience.
Profiles owns visibility, status, locale selection, and public presentation decisions. Forum does not
copy those rules or read Profiles tables directly.

When anonymous presentation is allowed, Search may contain only:

- `user_id`;
- `handle`;
- `display_name`;
- `avatar_media_id`.

The public handle also populates the Search `handle` column, and the handle/display name extend
keywords. Profile tags, preferred locale, visibility, biography, banner, and other owner data are not
serialized.

When the profile is absent, hidden, private, or otherwise denied, `payload.author` is `null`, the
Search handle is empty, `facets.author_id` is absent, and `facets.has_public_author` is false. A
Profiles owner failure is not converted into an empty author; projection fails so the durable inbox
can retry.

## Durable Profiles mutation redaction

The A1–A7 slices made public-summary changes durable and atomic:

- visibility, handle, localized content, preferred locale, and media updates persist the owner write
  and `ProfileUpdated` in one Profiles transaction;
- self-service upsert couples profile, translation, taxonomy-backed tags, and the outbox envelope;
- CLI backfill creation always emits the event for non-dry-run writes, uses a system actor, and
  preserves create-if-missing behavior;
- `--emit-events` remains accepted as compatibility input but can no longer disable invalidation.

A8 introduced a production source gate for direct non-event `ProfileService` mutation calls. A9
published `ProfileMutationService` as the preferred event-aware facade and routed GraphQL
self-service plus CLI backfill through it. A10 added compiler deprecation diagnostics to the seven
legacy mutation methods while retaining their signatures for compatibility.

Those slices do not make every externally callable legacy method intrinsically event-aware or
compile-time private.

## FORUM-23A11: canonical account deletion redaction

The canonical Auth admin `delete_user` operation is a deactivation, not hard erasure. A11 makes its
Search redaction evidence owner-ordered inside one database transaction:

1. Auth locks and deactivates the tenant-scoped user.
2. Profiles hides the tenant-scoped profile with
   `redact_profile_for_account_deactivation_in_tx`; a missing profile is already a valid redacted
   owner state.
3. Active sessions are revoked and the durable RBAC generation is reserved.
4. Auth persists `DomainEvent::UserDeleted` through `TransactionalEventBus::publish_in_tx` with the
   authenticated administrator as actor.
5. The transaction commits only after the outbox envelope is stored.

If Profiles redaction or durable event publication fails, the account deactivation transaction does
not commit. Event publication failure explicitly rolls back the Auth status change, profile hiding,
session revocation, and RBAC generation reservation.

`UserDeleted` is sufficient deletion-redaction evidence for this canonical path because the same
transaction has already hidden the Profiles owner row or proved it absent before storing the event.
It is not treated as an ungrounded post-commit notification.

Search maps both `ProfileUpdated` and `UserDeleted` to `forum_author:<user_id>`. Author scope remains
a redaction barrier and is not stale-skipped against an unrelated full-Forum wall-clock watermark.
The consumer rebuilds from current owner state; the hidden or absent profile therefore produces a
null public author in all surviving Forum topic and approved-reply documents.

This slice intentionally does not cover arbitrary `update_user` status changes to inactive or banned.
Those paths revoke sessions and invalidate authorization, but need a separate explicit Profiles
redaction policy before they can claim the same author-redaction guarantee.

## Search shape

Topic and approved-reply documents use:

- `payload.author` for the bounded public summary or `null`;
- `facets.author_id` only when Profiles returned a public summary;
- `facets.has_public_author` as an explicit boolean;
- the Search `handle` column for the permitted public handle.

The raw `payload.author_id` value is not serialized. Category documents and the public Forum query
contract remain unchanged.

## Ordering boundary

The durable Forum inbox preserves tenant serialization, retry/backoff, dead-letter limits,
opportunistic reconciliation, and advisory locking. Author events are redaction barriers, but the
broader Forum projection still uses envelope timestamps and event identity. General owner-issued
monotonic ordering across every producer remains future work.

## Compatibility

A11 adds no event schema variant, database migration, dependency, or `Cargo.lock` change.
`UserDeleted` already existed in the canonical event registry. No Forum GraphQL/REST contract,
Search query API, Search document schema, or public Profiles read API changes.

The externally visible Auth delete operation remains a deactivation returning the same result; its
internal safety guarantee is stronger because deactivation cannot commit without Profiles redaction
and durable invalidation.

## Remaining FORUM-23 scope

- owner-issued monotonic projection revisions across Forum producers;
- explicit policy for non-delete user status disabling paths;
- removal or compile-time restriction of deprecated `ProfileService` mutation APIs;
- bounded category-subtree, tag, locale, date, solved, kind, channel/group, attachment, and
  remaining author filters;
- member Search projections;
- maintainer-executed PostgreSQL redaction, rebuild, and query evidence.

## Maintainer verification

Not run by the implementation agent, per maintainer instruction.

Suggested commands:

```bash
node scripts/verify/verify-forum-search-account-deletion-redaction.mjs
node scripts/verify/verify-forum-search-public-author-summary.mjs
node scripts/verify/verify-forum-search-projection.mjs
cargo check -p rustok-profiles --all-targets
cargo check -p rustok-search --all-targets
cargo check -p rustok-server --all-targets
cargo test -p rustok-profiles account_redaction -- --nocapture
cargo test -p rustok-search forum_inbox -- --nocapture
cargo test -p rustok-search ingestion -- --nocapture
cargo xtask module validate profiles
cargo xtask module validate forum
```
