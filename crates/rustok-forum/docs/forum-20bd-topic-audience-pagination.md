# FORUM-20BD — exact topic audience pagination owner

`FORUM-20BD` publishes an exact storefront topic-list owner and composes the authenticated GraphQL unread-topic query through it.

## Delivered boundary

- `ForumTopicAudienceListService` scans the canonical base storefront candidate set in bounded pages of `MAX_FORUM_READ_LIMIT`.
- Every candidate is evaluated through `ForumTopicAudienceVisibilityService` before output pagination.
- The requested page items and `total` are derived from the same ordered allowed sequence, so hidden richer-audience topics cannot create sparse pages or leak through a pre-audience count.
- Authenticated reads accept one validated `PortContext`; tenant, actor, locale, channel, claims, deadline, and correlation identity remain transport-derived.
- `ForumStorefrontReadStateService::list_topics_with_unread_audience_visible` enriches only exact allowed topic IDs through the canonical unread aggregate.
- `forumStorefrontUnreadTopics` now constructs a `TopicList` read context and uses the host-published optional Forum audience facts capability through `ForumGraphqlRuntimeData`.
- Missing trust, Channel, or Groups facts fail closed only when a configured selector still requires them. Locally decidable role and explicit-user layers do not require the optional provider.

## Compatibility and degraded mode

The legacy `ForumStorefrontReadStateService::list_topics_with_unread` method remains available for consumers not yet migrated to exact audience context. No request or response DTO field, GraphQL field name, migration, dependency, or UI model changed.

The owner uses bounded database scan pages but computes an exact allowed total for the resolved candidate sequence. This slice does not claim snapshot isolation across concurrent topic mutations; PostgreSQL concurrency evidence remains part of the parent `FORUM-20` completion scope.

## Explicitly not delivered

`FORUM-20BD` does not yet compose the native storefront topic/unread paths or the public GraphQL `forumStorefrontTopics` path through the new owner. Exact reply/reply-list reads, category reads, search/index, SEO, deep links, visibility-scoped category/all-read commands, and canonical plan/`CRATE_API.md` synchronization remain follow-up work beginning with `FORUM-20BE`.

## Validation handoff

Tests, Cargo commands, formatting, verifiers, workflows, and CI were not run by the implementation agent, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum topic_read_transport -- --nocapture
cargo test -p rustok-forum --test topic_audience_exact_read_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-audience-pagination.mjs
node scripts/verify/verify-forum-topic-audience-transport-composition.mjs
cargo xtask module validate forum
```
