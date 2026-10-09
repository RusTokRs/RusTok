# rustok-forum

## Purpose

`rustok-forum` owns the forum domain with forum-owned persistence.

## Responsibilities

- Provide `ForumModule` metadata for the runtime registry.
- Own Forum category membership/bindings, topic/reply state, and moderation workflows; canonical Category identity, localized copy, hierarchy, routes and presentation are owned by `rustok-taxonomy`.
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
- Own stable localized topic routes and locale-aware category route identity with immutable historical slug reservations while keeping route resolution separate from visibility authorization.
- Expose visibility-safe category route resolution through additive GraphQL and native storefront transports without mounting a public category URL.
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
- Depends on `rustok-taxonomy` for canonical Category identity, localized copy, hierarchy, routes and presentation, and for the shared scope-aware term dictionary behind forum topic tags.
- Category slugs are translation-local. `ForumCategoryRouteService` owns the
  transport-neutral flat `/{locale}/forum/c/{slug}` identity, shared locale
  fallback semantics and immutable old-slug redirects. Historical
  `(tenant, locale, slug)` keys are permanently reserved and cannot be reclaimed
  by the original or another category. GraphQL and native storefront transports
  recheck the canonical category through the exact audience and channel boundary,
  but no public category URL is mounted yet. Topic routes use
  `/{locale}/forum/t/{short_id}/{slug}` with immutable redirect/tombstone history
  and a Rust storefront mount; ID routes remain compatibility paths.
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
- Declares the runtime module dependency set `content`, `media`, `taxonomy` in
  `rustok-module.toml`, `modules.toml`, and `RusToKModule::dependencies()`; the
  xtask manifest-parity validator keeps the three artifacts aligned (`tenant` is
  intentionally not a module dependency: the tenant module is a required core
  module and Forum reaches tenant data only through `rustok-api` host seams).
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

## Settings status

The manifest setting `use_reactions` is the canonical Forum engagement intent.
Forum resolves that intent through the tenant-module runtime API rather than importing
the tenant persistence entity directly. When the setting is enabled, effective
reaction mode still requires the Reactions module to be enabled; otherwise Forum stays
on internal voting. Provider absence affects only the selected engagement feature and
never disables Forum core behavior.

See the canonical settings contract in
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

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
- `graphql::GqlForumStorefrontCategoryRouteResolution`
- `graphql::MergeForumTopicGraphqlInput`
- `graphql::ResolveForumTopicMergeSolutionGraphqlInput`
- `graphql::GqlForumTopicMerge`
- `graphql::GqlForumTopicMergeSolutionResolution`
- `controllers::axum_router`
- `admin::ForumAdmin` (publishable Leptos package)
- `storefront::ForumView` (publishable Leptos package)

## Known Limitations / Pending Implementation

The items below are verified gaps. They are not hidden behind compatibility
wrappers. Each one has an owner decision or a follow-up task in
[`docs/implementation-plan.md`](./docs/implementation-plan.md).

- Topic reads (GraphQL `forumTopic`, `forumTopics`, REST topic list and detail,
  the REST redirect, and the public SEO provider) apply the topic-level and
  category audience contract. See
  [`DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`](../../../DECISIONS/2026-10-09-forum-topic-owner-audience-read.md).
- Vote and subscription writes evaluate the owner audience of the parent topic
  before the write, and reject self-voting unless `allow_self_voting` is enabled. See
  [`DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md`](../../../DECISIONS/2026-10-09-forum-vote-subscription-write-audience.md).
  Known gaps: the route channel is not part of the write gate; the gate runs
  before the write transaction, so a policy change committed in between can let
  one write through; clear operations are intentionally not gated.
- Widget previews (`forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`)
  apply the owner audience contract for the caller. See
  [`DECISIONS/2026-10-09-forum-widget-preview-owner-audience.md`](../../../DECISIONS/2026-10-09-forum-widget-preview-owner-audience.md).
  The topic-list preview scans at most 1000 candidate topics. Larger candidate sets
  fail until `category_id` narrows them. Previews do not take a route channel.
- The `Authoring` SEO scope keeps the system context and does not apply the
  audience contract.
- Moderation solution write responses (REST and GraphQL mark/clear) read the topic through
  the owner path after the write. The moderation gate exempts the exact topic author, so an
  author who has lost audience access gets `TopicNotFound` (404) after a committed write.
  See [`DECISIONS/2026-10-09-forum-topic-owner-audience-read.md`](../../../DECISIONS/2026-10-09-forum-topic-owner-audience-read.md).
- `ForumOwnerExportReader` is an operator-scope library reader. It is not
  audience-filtered and has no CLI or API transport.
- Among the `ForumModuleSettings` values, `use_reactions`, `allow_downvotes`, and
  `allow_self_voting` are enforced on internal vote writes. See
  [`DECISIONS/2026-10-09-forum-vote-policy-settings.md`](../../../DECISIONS/2026-10-09-forum-vote-policy-settings.md).
- Three `ForumModuleSettings` fields are declared in `rustok-module.toml` and `dto/settings.rs`, and no runtime code
  reads them yet. Changing them has no effect: `default_topic_sort` (the list order is fixed, see the topic sort item),
  `allow_user_topic_closing` (authors cannot close or lock their own topics through this setting), and
  `allow_anonymous_reading` (no read path checks it, and the admin form no longer shows it). Each one needs its own
  wiring decision, because each changes a read or write path.
  The unread settings removed by `DECISIONS/2026-10-09-forum-remove-unwired-module-settings.md` are no longer declared.
- `topics_per_page` and `replies_per_page` are the default page size for REST and GraphQL list requests that omit
  `per_page`. An explicit `per_page` is bounded to 1..=100 and wins over the setting. See
  [`DECISIONS/2026-10-09-forum-list-page-size-from-settings.md`](../../../DECISIONS/2026-10-09-forum-list-page-size-from-settings.md).
- Topic title and post body length limits are enforced on topic create, topic update (title or body
  supplied), topic translation upsert, reply create, and reply update (content supplied). Imports and exact
  translation apply are not limited. The check counts characters of the trimmed title and of the plain text of
  the body; `0` as a maximum means unlimited. The admin form edits only the title limits; body limits are
  set through the settings JSON. See
  [`DECISIONS/2026-10-09-forum-content-length-enforcement.md`](../../../DECISIONS/2026-10-09-forum-content-length-enforcement.md).
- `max_edit_window_minutes` is enforced on author topic and reply updates. After the window closes, an author
  update returns `FORUM_VALIDATION_FAILED` (HTTP 400). Moderators and administrators are not limited. See
  [`DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md`](../../../DECISIONS/2026-10-09-forum-wire-author-edit-window-and-locked-list-settings.md).
- Author self-service: the built-in Customer role edits its own topics and replies (`forum_topics:update` and
  `forum_replies:update`, scope `Own`). A role without the update or delete permission gets no author access, and any
  other user is refused. Authors delete their own topics and replies only when `allow_user_content_deletion` is `true`
  (default `false`). Moderators and administrators delete through their own permissions and are not affected. The
  checks are in the owner services, after the ownership check. See
  [`DECISIONS/2026-10-09-forum-author-self-service-and-deletion.md`](../../../DECISIONS/2026-10-09-forum-author-self-service-and-deletion.md).
- `pre_moderation_enabled = true` holds every new topic and reply as `Pending` until a moderator approves it. The
  per-category `moderated` flag still holds replies only, as before. A topic in `Pending` accepts no replies
  (`FORUM_TOPIC_AWAITING_MODERATION`). A pending topic or reply is visible only to its author and to viewers with the
  moderation scope `All`, on every owner read and list. Storefront reads and storefront search return only `Open` topics
  and `Approved` replies, so held content is excluded there. Moderators approve a pending topic with the reopen
  transition (`Pending` to `Open`, requires `ForumTopics:Moderate`) and reject it with the archive transition
  (`Pending` to `Archived`). A pending topic cannot be promoted to a Blog post. Known limitation: approving a held topic
  does not notify its subscribers, and there is no dedicated approve or reject transport for topics yet. See
  [`DECISIONS/2026-10-09-forum-topic-pre-moderation.md`](../../../DECISIONS/2026-10-09-forum-topic-pre-moderation.md) and
  [`DECISIONS/2026-10-09-forum-reply-pre-moderation-setting.md`](../../../DECISIONS/2026-10-09-forum-reply-pre-moderation-setting.md).
- `show_locked_topics_in_lists = false` hides locked topics from the storefront topic list. The widget preview
  (`forum.topic_list`) does not apply it yet, and owner and moderator lists are not filtered.
- Topic and reply creates enforce the per-author cooldowns `rate_limit_new_topic_seconds` and
  `rate_limit_new_reply_seconds` (`0` disables each). A create inside the cooldown returns `FORUM_RATE_LIMITED`
  (HTTP 429). Only user actors are limited. System and service actors, including the starter import and
  the starter driver, are exempt. The cooldown does not count window limits; see
  [`DECISIONS/2026-10-09-forum-posting-rate-limits.md`](../../../DECISIONS/2026-10-09-forum-posting-rate-limits.md).
  The posting policy evaluator (`posting_policy_evaluator.rs`) is not yet wired into the write path, so its
  window rules are not enforced.
- User reports on topics and replies are filed with the Moderation owner through the `reportForumTopic` and
  `reportForumReply` GraphQL mutations, which return the report id. Forum checks the reporter's read audience, rejects
  self-reports, and pins the current subject revision; the host adapter forwards the report through
  `ForumModerationReportPort`. Without that adapter the mutations fail closed. Not yet implemented: automatic flags
  (a `rustok-moderation` policy, not a forum setting), free-text descriptions, report rate limits, REST transport, and the storefront report
  action. See [`DECISIONS/2026-10-09-forum-user-reports-via-moderation.md`](../../../DECISIONS/2026-10-09-forum-user-reports-via-moderation.md).
- Notifications are emitted only for topic creation and user mentions.
- Topic move (`FORUM-21A`) has an owner service but no transport or UI.
  Topic revision history has services but no API or UI.
- Attachment relations have a service but no transport or UI.
- Topic view counters and topic sort orders are not implemented. The list order is
  fixed as pinned first, then recent activity.
- The Next.js storefront shows the first page of topics without load-more.
  Reply pagination has no load-more.

## Roadmap

[`docs/implementation-plan.md`](./docs/implementation-plan.md) is the only
authoritative forum roadmap and task-status source. Do not copy its task ledger
into README files, issues, or additional planning documents.

## Docs

- [Module docs](./docs/README.md)
- [Canonical implementation plan](./docs/implementation-plan.md)
- [Accepted Forum slug/locale decision](../../../DECISIONS/2026-03-29-forum-slug-locale-contract.md)
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
- [Category slug alias owner](./docs/forum-24m-category-slug-alias-owner.md)
- [Category route storefront transport](./docs/forum-24n-category-route-storefront-transport.md)
- [Platform docs index](../../../docs/index.md)
