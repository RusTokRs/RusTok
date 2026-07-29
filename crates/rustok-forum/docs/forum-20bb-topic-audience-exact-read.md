# FORUM-20BB exact storefront topic audience read

Status: source-ready / unvalidated.

This note records the delivered owner boundary only. The canonical Forum roadmap
remains [`implementation-plan.md`](implementation-plan.md).

## Delivered

- `ForumTopicAudienceReadService` publishes one exact public storefront topic read
  and one exact authenticated storefront topic read.
- Both paths require `forum_topics:read` before visibility evaluation.
- The owner composes the existing base topic rule with every inherited category
  audience layer and the optional topic-local layer through
  `ForumTopicAudienceVisibilityService`.
- Missing, foreign, closed, route-channel-denied, category-denied,
  richer-audience-denied, and concurrently unavailable topics all resolve as
  absent rather than exposing which predicate rejected the target.
- Public reads use `ForumTopicAudienceViewer::public()` and never call optional
  trust, Channel, or Groups fact providers.
- Authenticated reads require one caller-supplied read-only `PortContext`.
  `ForumTopicAudienceViewer::authenticated` validates the exact tenant and user
  before topic lookup or optional owner-facts access.
- The effective locale and route channel come from that exact context. The owner
  does not accept a second authenticated locale or channel argument that could
  disagree with the transport snapshot.
- Locally decidable role and explicit-user decisions remain provider-independent.
  Trust, Channel, and Groups selectors resolve only their requested exact facts;
  a missing still-required provider fails closed with the existing typed
  capability error.
- Topic localization/hydration occurs only after the canonical visibility owner
  returns an allowed decision.

## Source-ready proof

`topic_audience_exact_read_sqlite` covers:

- inherited root-role and child-trust conjunction;
- topic-local explicit deny;
- public fail-closed behavior without an owner-facts call;
- exact authenticated trust facts and successful hydration;
- low-trust and route-channel denial;
- provider absence;
- tenant/actor context mismatch before provider access;
- non-oracular missing-topic behavior;
- the minimal platform `users` fixture required by current Forum migrations.

## Boundary

- No existing `TopicService` method or transport call site changes.
- No topic-list, reply-read, category-read, search/index, SEO, or deep-link path
  is claimed as migrated.
- Native and GraphQL selected-topic composition, plus the authenticated
  mark-read visibility recheck, remain the separate `FORUM-20BC` slice.
- No migration, dependency, request/response DTO, GraphQL field, REST route,
  OpenAPI shape, or UI behavior changes.
- This is an authorization-time read snapshot, not a durable entitlement. Every
  later open, delayed delivery, or mutation must reauthorize through its owner
  boundary.

## Owner documentation handoff

This owner note is synchronized in the implementation slice. `CRATE_API.md` and
the canonical implementation plan are not replaced through the GitHub contents
API because both require complete-file replacement and the roadmap is
multi-thousand-line. A safe repository-local edit must record
`ForumTopicAudienceReadService`, advance the `FORUM-20` ledger through
`FORUM-20BB`, retain `FORUM-20BC` transport composition as the next bounded
slice, and keep list, reply, category, search/index, SEO, deep-link,
reconciliation, and runtime-proof work open.

## Verification status

The implementation agent did not run tests, Cargo commands, formatting,
verifiers, workflows, or CI. Suggested maintainer commands are recorded in
`forum-topic-audience-exact-read.json`.
