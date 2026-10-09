# Forum topic reads apply the full audience contract and the anonymous selected-topic field is removed

- Date: 2026-10-09
- Decision status: Accepted
- Implementation status: In progress
- Owners: `rustok-forum` (topic read services, GraphQL runtime, REST controllers, SEO target provider); `rustok-api` (`PortContext` and read-transport operations); consumers: `apps/next-admin`, `crates/modules/rustok-forum/admin`, `apps/next-frontend`, `crates/modules/rustok-forum/storefront`
- Extends: `DECISIONS/2026-09-29-cross-module-owner-diagnostic-ports.md`, `crates/modules/rustok-forum/docs/forum-20bc-topic-audience-transport-composition.md`
- Supersedes: None
- Superseded by: None

## Context

The forum audience contract (`contracts/forum-topic-audience-visibility.json`,
`ForumTopicAudienceViewer`, `ForumTopicAudienceVisibilityService`) evaluates the
base topic visibility, every inherited category audience layer, and the
topic-local layer. Several read paths did not apply it:

1. `forumTopic(id)` (GraphQL), `forum_topics` (GraphQL owner list), REST
   `GET /api/forum/topics` and `GET /api/forum/topics/{id}`, and the REST
   redirect used `TopicService::get_with_locale_fallback` or
   `list_with_locale_fallback`. These paths checked the category visibility
   floor only, so a topic restricted by a `minimum_trust_level`, role, or
   channel layer could be returned to a viewer who could not read it through
   the storefront audience path.
2. The anonymous GraphQL field `forumStorefrontTopic(id)` used the same
   service and was reachable without any audience context.
3. `TopicAudienceReadService::get_visible` checked the audience of the requested
   topic and returned the content of the canonical topic. After a merge, the
   content of the target could be disclosed through the source identifier
   without the target's audience being evaluated.
4. The topic provider for public SEO metadata and the sitemap candidate
   enumeration used a system `SecurityContext` and the base visibility floor.
   Titles and slugs of audience-restricted topics could appear in public
   metadata and in `sitemap.xml`.

## Decision

1. Every topic read that is exposed to an authenticated owner (GraphQL
   `forumTopic`, `forumTopics`, REST topic list and detail, REST redirect target)
   goes through the owner audience boundary:
   `ForumTopicAudienceReadService::get_authenticated_owner_visible_with_audience_context`
   and `ForumTopicAudienceListService::list_authenticated_owner_visible_with_audience_context`.
   The transport builds the `PortContext` with `topic_read_audience_port_context`
   (`ForumTopicReadTransport`, `ForumTopicReadOperation::TopicList` or
   `SelectedTopic`). The viewer is constructed with `ForumTopicAudienceViewer::authenticated`,
   which requires a user actor and matching `user_id`.
2. A topic denied by the owner audience boundary is reported as
   `ForumError::TopicNotFound` (single read) or omitted (list). It must not be
   distinguishable from an absent topic.
3. Canonical resolution checks the audience of the topic whose content is returned.
   When the requested topic redirects to a canonical topic, the canonical
   topic's audience is evaluated; the requested topic's audience is not sufficient.
4. The anonymous GraphQL field `forumStorefrontTopic` is removed. The
   storefront consumers use `forumStorefrontAudienceTopic` and
   `forumStorefrontAudienceTopics`. No compatibility wrapper is kept (zero-legacy).
5. Public SEO topic metadata and sitemap candidates use the public storefront
   audience methods (`get_public_storefront_visible_with_locale_fallback`,
   `list_public_storefront_visible_with_locale_fallback`). The `Authoring` SEO
   scope keeps the system context and is recorded as a known limitation in the
   crate README until the authoring preview gets its own audience contract.
6. REST `GET /api/forum/topics/{id}` is kept and now enforces the owner audience
   boundary. It is not removed in this decision.

## Sources of truth and ownership

- Canonical owner of topic visibility: `rustok-forum` (`ForumTopicAudienceVisibilityService`).
- Category audience policy: `rustok-forum` (`ForumCategoryAudiencePolicy`); category identity remains in `rustok-taxonomy`.
- Trust, channel membership, and group facts: the host-published `SharedForumAudienceFactsPort` (`rustok-api`); the forum module does not own those facts.
- Transport representations (GraphQL, REST, Leptos server functions, SEO providers) are projections and must not compute their own audience decision.
- Dependency direction is unchanged: `rustok-forum` depends on `rustok-api` ports; no module depends on the forum's transport layer.

## Invariants

- A topic whose audience the viewer does not satisfy is never returned by any
  owner or public read path, and never listed in any owner or public list.
- An audience denial and an absent topic produce the same outcome.
- The content returned for a canonical topic is always authorised by that
  canonical topic's audience.
- Authenticated owner reads require a user actor; anonymous reads use the
  public storefront method, not the owner method.

### Allowed states

- Owner read with an exact `PortContext` and the host facts port when a required
  richer layer needs trust, channel, or group facts.
- Public read with no facts capability for categories without richer layers.

### Forbidden states

- A public or anonymous read that returns a topic restricted by a richer layer.
- A list page that exposes a restricted topic or whose cursor skips a visible topic
  because a restricted topic was counted before filtering.
- Retaining `forumStorefrontTopic` as a compatibility field.

## Moderation solution write responses

`mark_topic_solution` and `clear_topic_solution` (REST, `controllers/moderation.rs`) and
`mark_forum_topic_solution` and `clear_forum_topic_solution` (GraphQL, `graphql/mutation.rs`)
return the topic after the write. The response read uses the owner path
(`get_authenticated_owner_visible_with_audience_context` with
`topic_read_audience_port_context(..., SelectedTopic)`), the same as the other post-write reads.

The moderation gate (`ForumModerationAudience::require_topic`) is skipped for the exact topic
author (`is_exact_topic_author`). Before this change the response read used the base path, so an
author who had lost access to a restricted topic could receive it after a successful write.
That is now prevented.

Consequence: an author who loses audience access and then marks or clears a solution gets
`TopicNotFound` (404) from the response read. The write has already been committed, so the
client sees a failed response for a successful change. This is accepted: fail closed beats leaking
a topic the owner rule already hides. The author exemption in the write gate is unchanged.

## Non-goals

- Vote and subscription writes: decided separately in
  `DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md`.
- Operator export (`ForumOwnerExportReader`): it requires manage scopes and is
  not an audience-filtered read.
- Topic-level `SEO Authoring` preview.

## Data, transaction, and concurrency boundary

Not applicable. The decision changes read paths only. No schema, migration, or
write transaction changes.

## Context dimensions

- Tenant: every owner and public call receives the tenant identifier from the typed `TenantContext`.
- Locale: the request locale is resolved by the existing `resolve_graphql_locale` or `request_context.locale` path and passed to the fallback chain.
- Channel: public reads pass the route channel slug; owner reads do not use the channel as a result filter, but the channel layer is evaluated when the topic carries one.
- Principal and auth: owner reads require `AuthContext`; public reads use `SecurityContext::public_read`.
- Policy: category audience constraints from `ForumCategoryAudiencePolicy`.
- Trace and deadline: `PortContext` carries a correlation identifier and a five-second read deadline.

## Events and projections

No events change. The SEO sitemap and metadata are derived projections; they are
recomputed on read and are not cached by this decision. Any cache that exists
upstream must be invalidated on audience policy change, which is tracked
separately.

## Failure semantics

- A missing trust or channel fact for a required layer fails closed (`PortError` mapped to denial).
- A facts capability that is absent for a required layer returns denial, not a degraded result.
- A denied canonical topic in a redirect returns `TopicNotFound`, not the source topic.

## Migration and cutover

- The first-party storefront adapters already request `forumStorefrontAudienceTopic`, so
  removing `forumStorefrontTopic` does not break first-party clients. A repository-wide
  search found no other consumer of the removed field.
- External GraphQL clients that still send `forumStorefrontTopic` receive a validation error.
  This is an intentional breaking change under the zero-legacy rule and must be noted in the release notes.
- REST responses for restricted topics change from 200 to 404 for viewers who previously
  received them through the base floor. The change tightens access and is not a
  compatibility regression.

## Alternatives considered

- Keep `forumStorefrontTopic` as a deprecated alias. Rejected: it would keep an
  anonymous path that bypasses the audience contract, and AGENTS.md forbids compatibility wrappers.
- Add an audience check only to REST and leave GraphQL unchanged. Rejected: the
  same data is reachable through three transports, and a partial fix leaves the GraphQL leak.
- Remove REST `GET /api/forum/topics/{id}`. Rejected for now: first-party admin
  transports still call it; enforcing the audience keeps the route safely.

## Verification

Scoped evidence for this decision:

- `scripts/verify/verify-channel-proof-points.mjs` (source locked, passes);
- `scripts/verify/verify-forum-topic-audience-transport-composition.mjs` (passes after the stale marker update);
- `scripts/verify/verify-forum-graphql-query-snapshot-cleanup.mjs` (checks its runtime markers; it still fails on `.github/workflows/forum11-diagnostics.yml`, which is absent at the base commit and is outside this change);
- new runtime tests in `crates/modules/rustok-forum/tests/topic_owner_audience_read_sqlite.rs`:
  owner single-read denial, owner list pagination after audience filtering, and public storefront list exclusion.

The Rust compilation and these tests were not executed in the authoring environment
because the Rust toolchain is not installed and crates.io is not reachable. They must
be run by `cargo check -p rustok-forum` and `cargo test -p rustok-forum` before merge.

## Consequences

- Authenticated owner reads through GraphQL and REST may return fewer topics than before.
- SEO sitemap entries and metadata for audience-restricted topics are removed.
- Follow-up work: the `Authoring` SEO scope, and a canonical-redirect runtime test with a real merge fixture.
