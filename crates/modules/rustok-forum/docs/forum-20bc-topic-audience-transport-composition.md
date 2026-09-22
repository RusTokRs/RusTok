# FORUM-20BC — exact topic audience transport composition

`FORUM-20BC` composes the module-owned native and GraphQL selected-topic reads,
and both authenticated mark-read transports, through the exact
`ForumTopicAudienceReadService` delivered by `FORUM-20BB`.

## Delivered boundary

- `topic_read_audience_port_context` derives the exact tenant, authenticated user,
  effective locale, route channel, permission claims, five-second read deadline,
  and bounded correlation identity from trusted native or GraphQL transport
  context. Topic IDs and locale remain request inputs; tenant and actor identity
  cannot be selected by a storefront DTO.
- `forumStorefrontAudienceTopic` is the module-owned GraphQL selected-topic field.
  Public requests use the public owner viewer and do not call optional trust,
  Channel, or Groups fact providers. Authenticated requests use the exact
  `PortContext` and the host-published `SharedForumAudienceFactsPort` when a
  still-required richer selector needs it.
- The storefront GraphQL adapter now requests
  `forumStorefrontAudienceTopic`. The older compatibility
  `forumStorefrontTopic` field remains available but is no longer the
  module-owned storefront selected-topic call site.
- The native server function constructs the same public or authenticated owner
  service from `HostRuntimeContext` and uses it for both an explicitly selected
  topic and the first topic selected from the current list response.
- `ForumStorefrontReadStateService::mark_topic_read_current_audience_visible`
  reuses the exact topic owner before any read-state write. Native and GraphQL
  mark-read transports both call this method with the same trusted context and
  optional facts capability.
- Missing, closed, route-channel denied, category-audience denied, topic-local
  denied, and absent topics remain non-oracular. Replies are not requested or
  returned when the exact selected-topic decision is unavailable.

## Compatibility and degraded mode

This slice adds no migration, dependency, request or response DTO field, route,
OpenAPI shape, or UI state. Categories without richer audience layers preserve
their previous behavior. Locally decidable roles and explicit-user rules do not
need an optional provider. A still-required trust, Channel, or Groups fact fails
closed when the host capability is absent.

The legacy `ForumStorefrontReadStateService::mark_topic_read_current_visible`
and the legacy GraphQL `forumStorefrontTopic` compatibility field remain for
consumers that have not yet migrated. New module-owned storefront selected-topic
and mark-read paths do not use them.

## Explicitly not delivered

`FORUM-20BC` does not migrate topic-list or unread-list pagination, exact reply
or reply-list reads, category reads, search/index, SEO, deep links, or
visibility-scoped category/all-read commands. Those remain bounded follow-up
work beginning with `FORUM-20BD`.

The canonical implementation plan and `CRATE_API.md` are not replaced through
the GitHub contents API in this slice. That API requires complete-file
replacement, and the canonical plan is multi-thousand-line; the conflict-safe
repository-local documentation synchronization debt remains explicit in the
machine contract.

## Validation handoff

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI, per maintainer request.

Suggested maintainer commands:

```text
cargo test -p rustok-forum topic_read_transport -- --nocapture
cargo test -p rustok-forum --test topic_audience_exact_read_sqlite -- --nocapture
node scripts/verify/verify-forum-topic-audience-transport-composition.mjs
node scripts/verify/verify-forum-topic-audience-exact-read.mjs
cargo xtask module validate forum
```
