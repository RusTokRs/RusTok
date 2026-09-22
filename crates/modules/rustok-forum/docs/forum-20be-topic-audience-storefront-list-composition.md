# FORUM-20BE — exact topic audience storefront list composition

`FORUM-20BE` composes the module-owned native storefront topic page and the
public GraphQL fallback page through the exact richer-audience pagination owner
delivered by `FORUM-20BD`.

## Delivered boundary

- The native storefront authenticated list derives a `TopicList` `PortContext`
  from the trusted tenant, authenticated user, request locale, route channel,
  permission claims, session identity, and five-second owner deadline.
- Authenticated native pages call
  `ForumStorefrontReadStateService::list_topics_with_unread_audience_visible`,
  so unread enrichment receives only exact audience-allowed topic IDs.
- Public native pages call
  `ForumTopicAudienceListService::list_public_storefront_visible_with_locale_fallback`.
- Both native paths derive `items`, the selected first topic, and `total` from
  the exact allowed page rather than the pre-audience candidate page.
- `forumStorefrontAudienceTopics` is an additive public GraphQL field backed by
  the same exact pagination owner. It validates optional tenant scope, reuses
  the route channel, and exposes the existing `ForumTopicConnection` shape.
- The canonical storefront GraphQL adapter keeps the authenticated exact unread
  query and switches only its public/personalization-unavailable fallback to
  `forumStorefrontAudienceTopics`.
- The canonical native and GraphQL adapters retain their already-migrated exact
  selected-topic and mark-read operations. Replies are still requested only
  after the selected-topic owner returns an allowed topic.

## Compatibility and degraded mode

No migration, dependency, existing GraphQL field, request DTO, response DTO, UI
model, transport selector, or compile-profile transport selection changes. The
canonical native and GraphQL adapter files now own both exact storefront reads
and their existing mark-read operations; no parallel adapter module remains.

Public topic decisions need no optional host facts. Authenticated native
locally decidable rules do not call the optional facts provider. Trust,
Channel, or Groups rules reuse the host-published facts capability and fail
closed when a still-required fact is unavailable.

## Explicitly not delivered

`FORUM-20BE` does not migrate exact reply or reply-list reads, category reads,
search/index, SEO, deep links, visibility-scoped category/all-read commands, or
PostgreSQL runtime evidence. Exact reply composition begins with `FORUM-20BF`.

The canonical `implementation-plan.md` and `CRATE_API.md` are not replaced
through the GitHub contents API in this slice. Their conflict-safe
repository-local synchronization debt remains explicit in the machine
contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum topic_read_transport -- --nocapture
cargo test -p rustok-forum --test topic_audience_exact_read_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-audience-pagination.mjs
node scripts/verify/verify-forum-topic-audience-storefront-list-composition.mjs
cargo xtask module validate forum
```
