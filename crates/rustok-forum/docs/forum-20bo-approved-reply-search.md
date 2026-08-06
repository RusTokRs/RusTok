# FORUM-20BO — approved reply Search documents

## Decision

The canonical FORUM-23 plan requires category, topic, reply and member index projections and limits public indexing to published or approved content. FORUM-20BO therefore publishes approved anonymous-public replies as independent `forum_reply` Search documents.

Reply text is not folded into the parent topic document. Keeping a separate identity preserves reply-level result kind, author and parent metadata, moderation removal and future FORUM-23 ranking/filter evolution.

## Forum-owned authorization

`ForumReplyAudienceReadService` now publishes an exact selected anonymous-public reply read. It resolves the typed allowed status and parent-topic visibility before loading reply content. `ForumPublicDiscoveryService` exposes that decision to cross-consumer projection code.

`crates/rustok-forum/src/search_projection.rs` remains the Forum-owned source mapper. Its direct reply-body query is limited to bounded raw locale candidate enumeration; it does not load reply status or content directly. Search receives only already-authorized `ReplyResponse` values.

A reply document is emitted only when all of the following are true:

- the selected reply owner accepts typed `ReplyStatus::Approved`;
- the parent topic passes the exact anonymous public visibility owner with no route channel;
- the parent category passes the exact anonymous public discovery owner;
- the selected owner returns the exact raw candidate locale rather than a fallback body.

This means pending, rejected, deleted, hidden or otherwise non-approved replies are absent. Replies beneath closed, channel-restricted, inherited-policy-denied or topic-policy-denied parents are also absent. Search SQL contains no Forum audience or moderation predicate.

The full source cursor now advances in three bounded phases:

1. category locale candidates;
2. topic locale candidates;
3. reply locale-body candidates.

Cursor progress follows raw candidates, so hidden or unapproved replies may produce sparse or empty pages without stopping a rebuild.

## Document contract

Each projected reply uses:

- `source_module = forum`;
- `entity_type = forum_reply`;
- key `forum_reply:{reply_id}:{locale}`;
- reply body as the searchable body;
- topic title as the result title;
- category name as the subtitle;
- bounded payload containing reply, topic, category, author, parent and solution identities.

Vote totals and author profile fields are deliberately not copied. A safe author summary and richer FORUM-23 filters remain later owner-composition work.

## Invalidation and atomic replacement

Existing lifecycle events already carry both topic and reply identity:

- approved creation publishes `ForumTopicReplied`;
- moderation and deletion publish `ForumReplyStatusChanged`.

Both currently enter Search through a `forum_topic` refresh. FORUM-20BO makes that refresh rebuild the atomic staged Forum scope, so one Search transaction updates the topic and all child replies together. Topic visibility, copy, category context and solution changes therefore cannot leave stale child reply documents.

Reply body or quote-relation edits now publish the existing `forum_topic` reindex target in the same owner transaction. No new root event or reindex target string is introduced.

This fan-out is intentionally conservative. A later FORUM-23 owner-revision/inbox slice may replace it with ordered targeted topic-plus-reply batches after the required revision contract exists.

## Canonical navigation

FORUM-24Q supersedes the original UUID query navigation described by this historical slice. `ForumSearchProjectionSource` now obtains the exact localized parent-topic descriptor from `ForumTopicRouteService` and publishes:

```text
/{locale}/forum/t/{short_id}/{slug}?reply={reply_id}
```

`rustok-search` remains the single cross-transport result URL projection. It requires the owner-projected route and validates the canonical Forum source/entity pair, result and payload identities, exact locale, topic short identity, route shape and the one exact reply query. It does not rebuild the topic path or retain a UUID compatibility fallback.

Existing indexed reply documents with the retired UUID query route are non-navigable until a Forum Search reindex. This still does not add a standalone reply page, scrolling or focus behavior; `reply` remains an additive topic-open selection key.

The current route contract is documented in [FORUM-24Q Search canonical route cutover](./forum-24q-search-canonical-route-cutover.md).

## Consumer boundary

Published storefront searches filter on `is_public = TRUE`; they do not maintain a fixed status allowlist, so the typed `approved` status is not discarded after projection. GraphQL, native storefront and Search admin preview mappings are entity-generic and delegate URL projection to `canonical_search_result_url`.

Admin global search keeps its fail-closed domain allowlist. FORUM-20BO adds explicit mappings:

- `forum_category` requires `FORUM_CATEGORIES_READ`;
- `forum_topic` requires `FORUM_TOPICS_READ`;
- `forum_reply` requires `FORUM_REPLIES_READ`.

The canonical source must be `forum` or `rustok-forum`; spoofed source/entity pairs remain filtered out before display. No consumer creates a local reply URL fallback.

## Compatibility

This slice reuses `search_documents` and the existing staged Forum replacement. It adds no migration, workspace dependency, Cargo.lock change, REST or GraphQL field change, public DTO change, transport-local URL fallback, or FFA/FBA status promotion.

FORUM-24Q changes only the projected route value and requires reindexing existing Forum documents. Search storage, events, invalidation and result DTOs remain unchanged.

The large canonical implementation plan, Forum CRATE_API and Search local plan remain conflict-sensitive synchronization debt and are not rewritten through the GitHub Contents API in this slice.

## Remaining work

FORUM-20BP should continue FORUM-23 with owner revision ordering, durable inbox reconciliation and the remaining safe author/filter contract. Runtime evidence for PostgreSQL rebuild preservation, projection cleanup, approved replies, Search queries and exact-visible bulk reads remains maintainer-owned.

## Validation status

No tests, Cargo commands, formatting, verifiers, workflows or CI were run by the implementation agent.
