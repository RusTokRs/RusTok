# FORUM-21I/J canonical merged-topic resolution and HTTP redirect

## Status

`source_ready_maintainer_execution_pending`

FORUM-21I adds one bounded read-side policy beneath the planned `FORUM-21`
umbrella. A selected topic ID that previously became the archived source of a
successful FORUM-21B merge resolves through the immutable merge receipt ledger
to the terminal retained topic. FORUM-21J composes that owner evidence into the
existing ID-based REST topic route as an authorization-safe permanent redirect.

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
inside the original merge transaction and is append-only. FORUM-21I/J keeps
that ledger as the sole canonical source-to-target edge.

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

Each hop uses one database statement that reads both non-deleted topic existence
and at most two outgoing receipt edges under the same statement snapshot. A
concurrent merge therefore linearizes either before that hop or after it; the
resolver cannot combine a pre-commit edge result with post-commit topic state.

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
GraphQL fields and SEO target contracts do not change.

## REST permanent redirect

Forum public topic lookup remains ID-based at:

```text
GET /api/forum/topics/{id}
```

A direct canonical target keeps the existing `200` response and existing
`TopicResponse` body. A merged source now returns:

```text
308 Permanent Redirect
Location: /api/forum/topics/{canonical_topic_id}
Cache-Control: private, no-store
```

The `Location` value is tenant-relative because tenant identity remains owned by
the host request context rather than embedded into an owner-module path. An
explicit `locale` query parameter is preserved through
`url::form_urlencoded::Serializer`; an implicit locale selected from headers,
cookies or tenant fallback is not materialized into the redirect URI.

The route middleware follows these boundaries:

1. missing authentication is rejected by the existing extractor;
2. a caller without `forum_topics:read` passes to the existing handler, which
   retains its exact `forum_permission_denied` response and receives no
   canonical identity evidence;
3. an authorized caller resolves the canonical topic;
4. a direct target passes to the existing GET handler;
5. a merged source performs the same target locale/fallback hydration before
   emitting `Location`;
6. missing, hidden, invalid-locale and integrity failures return their existing
   non-redirect response without a `Location` header.

The middleware is attached to the GET method before PUT and DELETE methods are
registered on the same `MethodRouter`. Update, delete, vote, subscription,
moderation, reply and other commands continue to require their exact current
topic identity. A stale source ID never silently mutates the retained target.

`private, no-store` prevents a shared intermediary from retaining a redirect
that was authorized under one tenant or viewer context. This source-ready slice
does not introduce a host/domain cache key contract.

## OpenAPI boundary

The generated Forum OpenAPI path is owned by the real redirect middleware
symbol and records both outcomes:

- `200` with `TopicResponse` for the direct canonical target;
- `308` with `Location` and `Cache-Control` headers for a merged source.

The path shape is unchanged. There is no parallel REST endpoint and no second
response DTO.

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

The controller tests in
`crates/rustok-forum/src/controllers/topic_redirect.rs` are source-ready to
verify through a real Axum router and real SQLite owner merge:

- merged source GET returns `308` with the target ID;
- an explicit locale survives in the encoded `Location` value;
- redirect cache policy is `private, no-store`;
- direct target GET reaches the existing handler and returns target JSON;
- missing and forbidden reads expose no `Location`;
- PUT registered after the GET route layer does not execute redirect logic.

## Remaining FORUM-21 and FORUM-24 work

The canonical `FORUM-21` and `FORUM-24` entries remain `planned`. FORUM-21A
through FORUM-21J are bounded source-ready slices. Remaining work includes:

- maintainer execution and retained SQLite/PostgreSQL evidence;
- localized public routes, canonical storefront URLs, slug aliases and slug
  tombstones under FORUM-24;
- public/admin merge command transport composition;
- manager-selected resolution when both topics have accepted solutions;
- cross-category merge;
- split, fork and reply-range workflows.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-owner.mjs
node scripts/verify/verify-forum-topic-canonical-resolution.mjs
node scripts/verify/verify-forum-topic-http-redirect.mjs
cargo test -p rustok-forum --test topic_canonical_resolution_sqlite -- --nocapture
cargo test -p rustok-forum controllers::topic_redirect::tests -- --nocapture
```

No command above was run by the implementation agent, per maintainer request.
