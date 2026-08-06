# FORUM-24J topic route tombstone visibility owner

Status: **source-ready / maintainer execution pending**

## Scope

FORUM-24J adds the immutable owner state required to decide whether a deleted Forum topic route may
later be disclosed as `GONE` to an anonymous storefront request.

This slice intentionally stops at the owner boundary. GraphQL, native storefront transport and the
Rust storefront host continue to hide `GONE` and return the same public result as missing or denied
content. Public `410 Gone` composition is follow-up task FORUM-24K.

## Why the alias ledger is insufficient

`forum_topic_route_aliases` proves that a localized route existed and that its final route disposition
is `gone`. It does not prove that the topic was publicly visible immediately before deletion.
Disclosing every tombstone would leak formerly authenticated-only, richer-audience-restricted or
channel-restricted topics.

The snapshot is therefore written before route tombstones and before the topic thread is soft-deleted,
inside the existing `TopicService::delete` transaction.

## Captured owner facts

The snapshot derives one anonymous disclosure baseline from the existing Forum owners:

- the locked topic status must be `open`;
- the inherited category visibility floor must be public;
- every inherited category audience layer must admit `SecurityContext::public_read()`;
- the optional topic audience layer must admit the same public actor;
- the exact normalized `forum_topic_channel_access` set is captured separately.

The snapshot does not store roles, users, groups, trust thresholds, audience reasons, alias reasons,
locale data or route metadata. Richer audience rules are reduced to one boolean public-admission fact.

## Lock and transaction order

The delete owner acquires locks in this order:

1. tenant category-tree advisory scope;
2. exact topic-audience advisory scope;
3. exact topic row claim;
4. visibility snapshot write;
5. route tombstone write;
6. topic-thread soft delete.

Category visibility writes now participate in the same tenant category-tree lock already used by
category audience and topic audience policy owners. Topic channel, category and status changes are
therefore serialized against the snapshot through their existing owner locks or the locked topic row.

## Storage

Migration `m20260806_000025_add_forum_topic_route_tombstone_visibility` adds:

- `forum_topic_route_tombstone_visibility` — one immutable parent row per tenant/topic;
- `forum_topic_route_tombstone_channels` — the exact normalized route-channel set.

The parent seals the channel scope with both:

- `route_channel_count`;
- a lowercase SHA-256 digest over the ordered, length-delimited channel slugs.

Update and delete triggers reject mutation of both tables. A later child insert cannot broaden public
disclosure because every read recomputes and compares the immutable count and digest; mismatch fails
closed as a route-resolution conflict.

PostgreSQL and SQLite migration paths are included. No other database backend is admitted.

## Replay and historical behavior

The writer uses insert-if-absent semantics, then reloads and compares the complete parent and channel
set. When a parent already exists, the writer does not append missing channel rows. Any flag, count,
digest or channel-set drift aborts the delete transaction.

Topics deleted before this migration have no snapshot and remain hidden. There is no historical
backfill because current storage cannot reconstruct whether a deleted topic was publicly visible at
the deletion instant.

Same-topic historical slug aliases can later use the topic snapshot after deletion. Cross-topic merge
routes whose target disappears remain hidden unless a separate owner-approved proof covers that
source route history.

## Boolean read boundary

`ForumTopicRouteTombstoneVisibilityService::can_disclose_public_gone` returns only a boolean decision:

- missing snapshot: `false`;
- snapshot not publicly disclosable: `false`;
- unrestricted public snapshot: `true`;
- restricted snapshot without a routed channel: `false`;
- restricted snapshot with a nonmatching channel: `false`;
- restricted snapshot with an exact matching channel: `true`;
- malformed or unsealed stored scope: fail-closed owner error.

The method does not expose the stored channel list or any audience-policy data.

## Deliberate exclusions

FORUM-24J does not change:

- `forumStorefrontTopicRoute` GraphQL output;
- the module-owned native storefront route DTO;
- Rust storefront HTTP status mapping;
- the existing public collapse of `GONE` to `404`;
- route identity, alias identity or slug policy;
- category routes, canonical/hreflang metadata or SEO;
- semantic events, admin mutations or receipts.

## Verification handoff

No tests, verifiers, formatting, Cargo commands, migrations, workflows, registered-host runs or browser
scenarios were executed while preparing this source slice.

Maintainers can run:

```bash
node scripts/verify/verify-forum-topic-route-tombstone-visibility-owner.mjs
cargo test -p rustok-forum --test topic_route_tombstone_visibility_contract -- --nocapture
cargo test -p rustok-forum topic_owner::route_tombstone_visibility::tests -- --nocapture
cargo check -p rustok-forum --all-targets
```

## Next slice

FORUM-24K should consume only the boolean owner decision:

- expose `GONE` from GraphQL and native storefront transports only when authorized;
- retain `null`/hidden behavior for missing snapshots, private snapshots and channel mismatch;
- map authorized `GONE` to private `410 Gone` in the Rust storefront host;
- add transport and HTTP source contracts without exposing snapshot payloads.
