# FORUM-21I canonical merged-topic resolution

## Status

`source_ready_maintainer_execution_pending`

FORUM-21I adds one bounded read-side policy beneath the planned `FORUM-21`
umbrella. A selected topic ID that previously became the archived source of a
successful FORUM-21B merge now resolves through the immutable merge receipt
ledger to the terminal retained topic.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-canonical-resolution.json
```

Cumulative merge contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-owner.json
```

## One source of truth

No alias table, compatibility registry or parallel redirect model is added.
`forum_topic_merge_operations` already records the committed source and target
inside the original merge transaction and is append-only. FORUM-21I makes that
ledger the sole canonical source-to-target edge.

Migration
`m20260803_000017_add_forum_topic_canonical_resolution` adds:

- one unique `(tenant_id, source_topic_id)` edge;
- a PostgreSQL and SQLite insert guard requiring the source to be a non-deleted,
  archived, locked, zero-reply tombstone;
- a matching requirement that the target is non-deleted, non-archived and in the
  receipt category.

A retained target may later become the source of another merge. This forms a
forward chain without allowing one source identity to branch to multiple
canonical targets.

## Bounded resolution

`TopicService::resolve_canonical_topic` returns:

- the requested topic ID;
- the terminal canonical topic ID;
- whether the request was redirected;
- the bounded hop count;
- immutable merge operation IDs in traversal order.

The resolver follows at most 32 edges. Duplicate source edges, cycles, hop
exhaustion or other ambiguous history fail closed with:

```text
FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT
```

The terminal topic must still exist in the tenant and must not be soft-deleted.
An unknown unmerged ID returns the original `FORUM_TOPIC_NOT_FOUND` boundary.

## Selected read cutover

Canonical resolution is composed into the public topic owner facade before
selected-topic hydration:

- `TopicService::get` and `get_with_locale_fallback` return the terminal target
  representation;
- `get_with_canonical_resolution_and_locale_fallback` also returns traversal
  evidence for transports that need the requested-versus-canonical distinction;
- storefront selected-topic reads evaluate category and topic visibility against
  the canonical target, never the archived source tombstone;
- GraphQL `forumTopic` inherits the same target resolution because it already
  uses `TopicService`;
- Forum SEO target load and route resolution inherit the same target identity
  through the same owner path.

The returned `TopicResponse.id` is the terminal target ID. Response shapes,
GraphQL fields, REST path parameters and SEO target contracts do not change.

## REST boundary

Forum public topic lookup remains ID-based. `GET /api/forum/topics/{id}` now
returns the canonical target representation when `{id}` is a merged source.
This slice deliberately does not emit an HTTP 3xx status or `Location` header.

A later route-focused slice may add permanent HTTP redirects and slug aliases
once the public URL contract is explicitly selected. FORUM-21I does not invent
that route model inside the domain resolver.

## Mutation boundary

Only selected reads follow canonical merge history. Update, delete, vote,
subscription, moderation and reply commands continue to require their exact
current topic identity. This prevents a stale source ID from silently mutating a
retained target without an explicit command-transport policy and authorization
review.

## Failure mapping

Canonical history inconsistency is an internal integrity failure. REST maps
`FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT` to a generic HTTP 500 response;
public messages do not expose storage rows, SQL or traversal details. The error
is non-retryable because duplicate or cyclic immutable history requires an
explicit operator repair rather than automatic request replay.

## Source-ready regression

`crates/rustok-forum/tests/topic_canonical_resolution_sqlite.rs` is source-ready
to verify:

- a two-edge `A -> B -> C` chain resolves both archived source IDs to `C`;
- traversal evidence retains operation IDs in chain order;
- direct lookup of `C` has zero hops;
- selected owner read returns the target ID and target content;
- storefront visibility is evaluated for the target;
- an unknown ID remains not found;
- a second edge for the same source is rejected;
- a direct receipt with an active source is rejected.

## Remaining FORUM-21 work

The canonical `FORUM-21` entry remains `planned`. FORUM-21A through FORUM-21I
are bounded source-ready slices. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- an explicit public HTTP redirect and slug/route tombstone contract;
- public/admin merge transport composition;
- manager-selected resolution when both topics have accepted solutions;
- cross-category merge;
- split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-canonical-resolution.mjs
cargo test -p rustok-forum --test topic_canonical_resolution_sqlite -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
