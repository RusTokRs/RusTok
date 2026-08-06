# rustok-forum

## Purpose

`rustok-forum` owns the forum domain with forum-owned persistence.

## Responsibilities

- Provide `ForumModule` metadata for the runtime registry.
- Own forum categories, topics, replies, and moderation workflows.
- Own forum subscriptions through forum-owned `forum_category_subscriptions` and
  `forum_topic_subscriptions`.
- Own per-user forum statistics through forum-owned `forum_user_stats`.
- Own forum voting state through forum-owned `forum_topic_votes` and `forum_reply_votes`.
- Own accepted-solution workflow for Q&A-style topics through forum-owned `forum_solutions`.
- Own forum topic tag attachments through forum-owned `forum_topic_tags` while reusing
  `rustok-taxonomy` as the shared term dictionary.
- Own forum topic donor payload in `forum_topics.metadata`, including the live attached-mode
  Flex integration for locale-aware custom fields through parallel localized records.
- Apply module-owned reply lifecycle rules, including pending replies for moderated categories and approved-only public storefront reads.
- Own forum storage tables for categories, topics, translations, replies, and channel access through `forum_topic_channel_access`.
- Expose shared multilingual contract fields on forum read surfaces:
  `requested_locale`, `effective_locale`, and `available_locales`.
- Own stable localized topic routes and locale-aware category route identity while keeping route resolution separate from visibility authorization.
- Own forum GraphQL and REST transport adapters alongside the domain services.
- Keep REST category/topic/reply/user/widget handlers on narrow `ForumHttpRuntime` state; the manifest-declared Axum router builds it from `HostRuntimeContext` and a typed transactional event bus.
- Publish the forum widget contract-freeze catalog/validation surfaces (`ForumWidgetContractService`, `/api/forum/widgets/catalog`, `/api/forum/widgets/validate`, `forumWidgetCatalog`).
- Maintain page-builder consumer evidence for FW-2 fallback hardening and the live Wave 1 rollout packet, including static no-compile verification of fallback profiles, smoke outcomes, read-path no-5xx guarantees, numeric SLO thresholds, forum-owned observability traces, rollback decision, owner approvals, waiver-free evidence, monthly refresh/stale-rollout-block policy, non-empty required refresh sections, and machine-readable latest-refresh provenance.
- Publish a module-owned Leptos admin UI package in `admin/` for host composition.
- Publish a module-owned Leptos storefront UI package in `storefront/` for host composition.
- Publish the typed RBAC surface for `forum_categories:*`, `forum_topics:*`,
  and `forum_replies:*`.

## Interactions

- Depends on `rustok-content` for shared rich-text, locale, and future orchestration helpers.
- Depends on `rustok-taxonomy` for the shared scope-aware term dictionary behind
  forum topic tags.
- Category slugs are translation-local. `ForumCategoryRouteService` owns the
  transport-neutral flat `/{locale}/forum/c/{slug}` identity and shared locale
  fallback semantics, but no public category route is mounted yet. Topic routes
  use `/{locale}/forum/t/{short_id}/{slug}` with immutable redirect/tombstone
  history and a Rust storefront mount; ID routes remain compatibility paths.
  Every route transport must still apply the exact Forum audience, channel and
  module visibility owner before disclosure.
- A selected merged-source ID resolves through the immutable
  `forum_topic_merge_operations` chain to the terminal retained topic.
  `GET /api/forum/topics/{id}` returns an authorization-safe `308 Permanent Redirect`
  for a merged source and keeps the existing `200 TopicResponse` for a direct target.
- The manager-only GraphQL mutation `mergeForumTopic` composes the idempotent
  `ForumTopicMergeService` owner, derives tenant authority from the routed
  request, requires `forum_topics:manage`, and returns the immutable merge
  receipt instead of hydrating a topic response.
- `mergeForumTopicResolvingSolution` composes the same owner transaction for two
  valid competing accepted solutions. The manager selects one exact accepted
  reply ID; the winning marker metadata is preserved, the losing author receives
  one exact solution-count decrement, and an append-only
  `forum_topic_merge_solution_resolutions` row records the decision through the
  immutable merge receipt.
- Both merge commands support same-category and checked cross-category ownership.
  A cross-category merge keeps both topic identities in their original categories,
  archives the source as a canonical tombstone, and transfers only the exact
  published-reply aggregate from source category to target category with checked,
  fail-closed arithmetic.
- `m20260803_000019_allow_cross_category_topic_merge_redirect_edges` permits the
  archived source tombstone to differ from the receipt target category while
  retaining all target-category, active-target and unique-source-edge guards.
- FORUM-21N composes the two manager merge mutations in the module-owned Leptos
  route `/modules/forum/merge` and Next-admin route `/dashboard/forum/merge`.
  Both keep one UUID operation identity across an exact retry, derive any selected
  accepted-solution reply from current candidate state, and display the immutable
  owner receipt.
- FORUM-21O selects direct authenticated native server functions for the Leptos
  merge route in SSR/hydrate builds while retaining the existing GraphQL adapter
  for CSR/headless builds. Native DTOs carry no access token or owner identity,
  and a failed selected transport never falls back to the other path.
- Resolved and ordinary merges retain the exact `forum.topic.merged` schema-1
  event contract so subscription, read-state, tag, vote and audience
  reconciliation owners remain unchanged.
- Negative solution-count transitions fail closed unless one existing positive
  contribution can be decremented atomically; they no longer silently saturate
  inconsistent state at zero.
- Shares SEO target ownership with `rustok-seo`: the shared SEO runtime now resolves
  `forum_category` and `forum_topic`, while owner-side SEO authoring stays embedded
  in `rustok-forum-admin`; public SEO for channel-restricted topics is resolved only
  when the host passes the matching request channel slug into the shared SEO contract.
- Depends on `rustok-channel` for public channel module gating and topic/reply/SEO visibility filtering with host-provided request channel slugs.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- `apps/admin` consumes `rustok-forum-admin` through manifest-driven `build.rs` code generation, with a NodeBB-inspired moderation workspace mounted under `/modules/forum`.
- `apps/storefront` consumes `rustok-forum-storefront` through manifest-driven `build.rs` code generation, with a public NodeBB-inspired discussion feed mounted under `/modules/forum`.
- Declares permissions via `rustok-core::Permission`.
- Transport adapters validate forum permissions against `AuthContext.permissions`, then pass
  a permission-aware `SecurityContext` into forum services.
- Forum services re-validate category/topic/reply/moderation permissions locally, so
  transport bugs cannot bypass forum mutation or moderation policy.
- Topic solution marking lives in forum-owned services and transport adapters; only
  approved replies can become solutions, and the read-path exposes `solution_reply_id`
  on topics plus `is_solution` on replies.
- Topic and reply voting lives in forum-owned services and transport adapters; the
  read-path exposes `vote_score` plus viewer-specific `current_user_vote`, while
  GraphQL/REST can set or clear votes without expanding the module permission surface.
- Category and topic subscriptions live in forum-owned services and transport
  adapters; the read-path exposes viewer-specific `is_subscribed`, and GraphQL/REST
  can subscribe or unsubscribe without introducing a new permission family.
- Per-user forum stats live in forum-owned services and transport adapters; the
  module tracks `topic_count`, `reply_count`, and `solution_count` through topic/reply
  lifecycle and accepted-solution transitions, and exposes a dedicated read-path for
  user-level stats.
- Topic tag write-paths resolve existing global taxonomy tags before creating
  new forum-local terms, while forum responses still expose the same `Vec<String>`
  tag contract.
- Topic metadata participates in the same multilingual attached-value contract as
  other live Flex donors: shared keys stay in `forum_topics.metadata`, locale-aware
  keys persist in `flex_attached_localized_values`, and read surfaces resolve them
  against the effective locale/fallback chain instead of treating topic custom fields
  as a schema-only concern.

## Entry points

- `ForumModule`
- `TopicService`
- `ReplyService`
- `CategoryService`
- `services::ForumCategoryRouteService`
- `ForumTopicRouteService`
- `ModerationService`
- `SubscriptionService`
- `UserStatsService`
- `VoteService`
- `ForumTopicMergeService::merge_topic`
- `ForumTopicMergeService::merge_topic_resolving_solution`
- `graphql::ForumQuery`
- `graphql::ForumMutation`
- `graphql::MergeForumTopicGraphqlInput`
- `graphql::ResolveForumTopicMergeSolutionGraphqlInput`
- `graphql::GqlForumTopicMerge`
- `graphql::GqlForumTopicMergeSolutionResolution`
- `controllers::axum_router`
- `admin::ForumAdmin` (publishable Leptos package)
- `storefront::ForumView` (publishable Leptos package)

## Roadmap

[`docs/implementation-plan.md`](./docs/implementation-plan.md) is the only
authoritative forum roadmap and task-status source. Do not copy its task ledger
into README files, issues, or additional planning documents.

## Docs

- [Module docs](./docs/README.md)
- [Canonical implementation plan](./docs/implementation-plan.md)
- [Accepted Forum slug/locale decision](../../DECISIONS/2026-03-29-forum-slug-locale-contract.md)
- [Merge owner](./docs/forum-21b-topic-merge-owner.md)
- [Checked cross-category merge](./docs/forum-21m-topic-merge-cross-category.md)
- [Accepted-solution policy](./docs/forum-21h-topic-merge-solution-policy.md)
- [Competing solution resolution](./docs/forum-21l-topic-merge-solution-resolution.md)
- [Canonical merged-topic resolution and HTTP redirect](./docs/forum-21i-topic-canonical-resolution.md)
- [Topic merge GraphQL transport](./docs/forum-21k-topic-merge-graphql-transport.md)
- [Admin topic merge workflow](./docs/forum-21n-topic-merge-admin-ui.md)
- [Native Leptos admin merge transport](./docs/forum-21o-topic-merge-native-admin.md)
- [Topic route identity owner](./docs/forum-24a-topic-route-identity-owner.md)
- [Authorized topic route gone transport](./docs/forum-24k-topic-route-authorized-gone.md)
- [Localized category route identity owner](./docs/forum-24l-category-route-identity-owner.md)
- [Platform docs index](../../docs/index.md)
