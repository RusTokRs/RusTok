# FORUM-23B2F4 trusted current-channel Search filter

## Status

`source_complete_execution_pending`

This slice adds one additive narrowing option to the explicit Forum-only
storefront Search path. When `currentChannelOnly` is active, Search returns only
Forum topics explicitly assigned to the trusted current request channel and
approved replies whose parent topic is explicitly assigned to that channel.

The implementation keeps the existing bounded candidate scan, Forum owner
eligibility, visible totals, facets and pagination in one execution owner. It
does not add another Search execution or normalization path.

## Trusted input boundary

The filter is a boolean, not a caller-selected channel slug. The existing
`TrustedStorefrontChannel` continues to derive the exact channel ID and slug from
`RequestContext` and treats the public `channel_id` input as an assertion only.

`currentChannelOnly: true` requires a complete trusted channel context. An
unscoped request fails validation instead of selecting a channel. `false` and an
absent argument preserve the previous Forum Search behavior, which may include
global topics plus topics visible in the trusted channel.

## Projection boundary

Forum remains the channel-assignment owner. Search evaluates only Forum-projected
values:

```text
forum_topic.payload.channel_slugs
forum_reply.payload.topic_channel_slugs
```

The reply value is copied from the current parent topic during Forum projection.
Legacy reply documents without `topic_channel_slugs`, malformed arrays and arrays
containing non-string values fail closed while the filter is active until a Forum
Search reindex repairs them.

## Parent-derived refresh ordering

A parent topic channel change must update both the topic document and every reply
document that copied `topic_channel_slugs`. The public root `TopicService::update`
therefore uses the existing `update_with_inline_relations` owner command rather
than the older compatibility update. That command writes
`ReindexRequested { target_type: "forum_topic" }` through the transactional
outbox before the topic transaction commits.

The Search inbox maps an exact `forum_topic` reindex request to the tenant-wide
Forum scope. `ForumSearchProjector::refresh_entity` also converts a topic refresh
to `rebuild_tenant`, so the resulting projection replaces the topic and all of
its approved replies from current Forum owner state. The same owner path preserves
omitted quote relations instead of clearing them during an ordinary topic edit.

This is source-locked ordering only. Runtime execution of the topic update,
outbox relay, ordered inbox claim and full projection rebuild remains maintainer
evidence.

## Evaluation order

The stable raw 100-candidate limit is checked before channel narrowing. Exact
locale, author, tag, solved, date and current-channel predicates intersect before
exact Forum topic/reply owner eligibility. Visible totals, facets, offset and
limit are computed after authorization while original ranking order is retained.

Categories and global topics do not match an active current-channel filter.
Query-rule pins are disabled while it is active because the pin loader has no
current-channel-only argument and must not reintroduce an out-of-scope document.

## Transport compatibility

Existing GraphQL operations and native endpoints retain their signatures. The
additive GraphQL operation is:

```text
ForumStorefrontSearchByCurrentChannel
```

The additive native endpoint is:

```text
search/forum-storefront-search-by-current-channel
```

Both select the same compile-profile transport and delegate to the shared Search
execution owner. No neutral `SearchQuery`, public `SearchPreviewInput`, shared
storefront filter DTO, migration, dependency or `Cargo.lock` change is introduced.

## Deferred boundaries

This slice intentionally does not expose arbitrary channel selection. A future
multi-channel selector requires a separately authorized owner contract rather
than accepting an untrusted slug.

Group filtering remains Forum-owned visibility policy and must not copy group
membership into Search payloads. Kind filtering remains blocked on the
`FORUM-22` topic-kind owner model. Attachment-presence filtering remains blocked
on `FORUM-14` attachment relations.

The full channel/group roadmap therefore remains open. The large canonical Forum
ledger still needs a focused synchronization that preserves unrelated concurrent
plan changes; this source slice records that remaining synchronization explicitly
instead of replacing the plan wholesale.

## Maintainer verification

The implementation agent did not run these commands, as requested:

```bash
cargo test -p rustok-search forum_current_channel_filter -- --nocapture
cargo test -p rustok-search current_channel_only -- --nocapture
cargo test -p rustok-forum topic_update -- --nocapture
cargo test -p rustok-search forum_projection -- --nocapture
cargo test -p rustok-search-storefront transport::tests::only_explicit_forum_category_scope_selects_owner_path -- --nocapture
node scripts/verify/verify-forum-search-current-channel-filter.mjs
node scripts/verify/verify-forum-projection-invalidation.mjs
cargo check -p rustok-forum --all-targets
cargo check -p rustok-search --features graphql --all-targets
cargo check -p rustok-search-storefront --features ssr --all-targets
cargo xtask module validate forum
cargo xtask module validate search
```

PostgreSQL and reindex evidence should cover a trusted channel, an unscoped
request, global and channel-assigned topics, approved replies, legacy reply
projections, malformed arrays, intersections with the existing filters, the raw
candidate cap, owner eligibility, totals, facets and pagination. It should also
change a topic channel assignment through the public owner, retain omitted quote
relations, observe one transactional `forum_topic` invalidation, process the full
scope in order and confirm that both topic and reply channel projections changed.
