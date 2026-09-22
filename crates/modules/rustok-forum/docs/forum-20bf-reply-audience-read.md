# FORUM-20BF — exact reply audience reads

`FORUM-20BF` publishes a reply-read owner that authorizes every reply through
its parent topic audience policy before reply content or pagination is returned.

## Delivered boundary

- `ForumTopicAudienceVisibilityService::is_topic_owner_visible` composes the
  inherited category floor and all richer category/topic audience layers without
  imposing storefront-only `open` or route-channel requirements. This preserves
  authorized owner/admin reads of closed topics.
- Existing `is_topic_visible` remains the storefront decision and still requires
  an open topic plus the matching route channel.
- `ForumReplyAudienceReadService` owns exact selected-reply, owner reply-list,
  and storefront reply-list reads. Missing and denied resources use the same
  absent semantics.
- Authenticated reply contexts derive tenant, user, locale, route channel,
  permission claims, session identity, correlation identity, and a five-second
  facts deadline only from trusted transport extensions.
- REST `GET /api/forum/replies/{id}` and
  `GET /api/forum/topics/{id}/replies` now call the exact owner.
- REST reply-vote commands perform an exact selected-reply preflight before the
  side effect and use the same owner for the returned reply.
- Additive GraphQL fields `forumAudienceReplies` and
  `forumStorefrontAudienceReplies` expose exact owner and storefront lists.
- The storefront transport selector replaces both native and GraphQL reply
  results with pages returned by the exact owner before `StorefrontForumData`
  reaches the UI.

## Compatibility

No migration, dependency, existing REST route, existing GraphQL field, request
DTO, response DTO, storefront model, or mark-read path changes.

The legacy `forumReplies` and `forumStorefrontReplies` fields remain available.
The existing base storefront adapters still perform their compatibility reply
fetch, but that result is replaced by the exact reply adapter before the final
storefront data is returned. Removing the duplicate fetch and migrating the
legacy authenticated field are explicit `FORUM-20BG` work.

Public and authenticated storefront reads continue to expose only approved
replies. Locally decidable audience layers do not call the optional facts port;
trust, Channel, or Groups rules fail closed when their required facts are not
available.

## Explicitly not delivered

`FORUM-20BF` does not migrate category reads, search/index, SEO, deep links,
visibility-scoped category/all-read commands, or PostgreSQL runtime evidence.

The canonical `implementation-plan.md` and `CRATE_API.md` are not replaced
through the GitHub contents API in this slice. Their conflict-safe
repository-local synchronization debt remains explicit in the machine contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum reply_read_transport -- --nocapture
cargo test -p rustok-forum --test topic_audience_exact_read_sqlite -- --nocapture
node scripts/verify/verify-forum-reply-audience-read.mjs
cargo xtask module validate forum
```
