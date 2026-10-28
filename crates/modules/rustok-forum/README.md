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
  and a Rust storefront mount; ID routes remain compatibility paths. The Next.js storefront serves the same prefix at
  `src/app/[locale]/forum/t/[...slug]`, so a link the owner authors resolves on both hosts.
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
- A same-category merge additionally runs the five topic-scoped follow-up
  reconciliations (subscriptions, read states, votes, tags, audience membership)
  inside the same transaction through each domain owner's idempotent core. The
  relations follow the retained target atomically with the merge receipt, keyed
  by deterministic operation ids derived from the merge operation, so an exact
  merge retry replays them as no-ops. Cross-category merges intentionally skip
  this stage: the relations are category-local and the owners reject category
  mismatches, so they stay stranded on the tombstone until a later operator
  repair (reconciliation entries fail closed instead of rewriting membership
  semantics silently).
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
  user-level stats. Topic archive/reopen now recomputes all affected author
  topic/reply/solution counts in the write transaction and adjusts category
  reply counts alongside topic counts. Solution mark/clear also reject an
  archived state after acquiring the topic row lock, rather than trusting only
  a pre-transaction read. Historical drift predating this change still requires
  operator reconciliation (FORUM-33).
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
  one write through. Domain topic clears are intentionally not gated, but category
  clears require category-owner visibility to avoid existence oracles. The
  canonical GraphQL `clearForumTopicSubscription` now acknowledges only `true`,
  while REST `DELETE /api/forum/topics/{topic_id}/subscription` returns
  `204 No Content` (both changed from full topic responses). They remove only
  the caller's own row, including when the topic is hidden or absent, without
  revealing content or a post-write 404. GraphQL category set/clear and topic
  set, and REST category set/clear, pre-read their owner-authorized response
  before writing and overlay the resulting `isSubscribed` / `is_subscribed` flag.
- Widget previews (`forum.topic_list`, `forum.topic_detail`, `forum.reply_stream`)
  apply the owner audience contract for the caller. See
  [`DECISIONS/2026-10-09-forum-widget-preview-owner-audience.md`](../../../DECISIONS/2026-10-09-forum-widget-preview-owner-audience.md).
  The topic-list preview scans at most 1000 candidate topics. Larger candidate sets
  fail until `category_id` narrows them. Previews do not take a route channel.
- Public SEO topic metadata and sitemap enumeration pass the static settings
  reader through both the audience wrapper and its topic mapper. Public SEO bulk
  pages use audience-visible keysets: topic cursors come from the owner list;
  category cursors come from the last visible category, never a hidden raw row.
  Selective policies can require scanning more raw candidates than the page size.
  The `Authoring` SEO scope keeps the system context and does not apply the
  audience contract.
- Topic/reply vote set/clear and topic solution mark/clear return only
  command acknowledgements: REST `204 No Content`, GraphQL `Boolean!`.
  There is no post-commit object read that could turn success into a 404 or
  disclose restricted content. Clients needing updated details must call a
  separately authorized owner read. See the write acknowledgement ADR.
- `ForumOwnerExportReader` is an operator-scope library reader. It is not
  audience-filtered and has no CLI or API transport.
- Among the `ForumModuleSettings` values, `use_reactions`, `allow_downvotes`, and
  `allow_self_voting` are enforced on internal vote writes. See
  [`DECISIONS/2026-10-09-forum-vote-policy-settings.md`](../../../DECISIONS/2026-10-09-forum-vote-policy-settings.md).
- `allow_anonymous_reading=false` now denies anonymous category/topic/reply discovery and
  routes, including SEO, Search projection and query-time eligibility of previously indexed
  category/topic/reply documents. The read-only static-module-settings port is required on
  public reads; an absent reader fails closed, whereas a missing settings *row* uses the
  backward-compatible default `true`. Authenticated reads still require their own RBAC,
  channel and audience decisions; native and Search storefronts never downgrade a
  signed-in caller with missing permissions to a guest. The control remains hidden
  in the admin form; saved `false` values are preserved. See
  [`DECISIONS/2026-10-15-forum-anonymous-discovery-and-category-counts.md`](../../../DECISIONS/2026-10-15-forum-anonymous-discovery-and-category-counts.md).
- Category `topic_count` and `reply_count` are public **global category aggregates**;
  they can include discussions outside the viewer's audience and must never be used as
  a viewer-visible list total or pagination cursor. The Next.js category UI labels
  this distinction; per-viewer listing still comes from the exact audience owner.
- Two declared `ForumModuleSettings` fields remain unread: `default_topic_sort`
  (the list order is fixed, see the topic sort item) and `allow_user_topic_closing`
  (authors cannot close or lock their own topics through this setting). Changing
  either has no effect. The unread settings removed by
  `DECISIONS/2026-10-09-forum-remove-unwired-module-settings.md` are no longer declared.
- `topics_per_page` and `replies_per_page` are the default page size for REST and GraphQL list requests that omit
  `per_page`. An explicit `per_page` is bounded to 1..=100 and wins over the setting. The module storefront transports
  and the Next.js forum package omit the argument, so a configured value reaches those surfaces too; the Next.js
  widget-preview helpers keep their documented `ForumWidgetLimit` because they render static pages. See
  [`DECISIONS/2026-10-09-forum-list-page-size-from-settings.md`](../../../DECISIONS/2026-10-09-forum-list-page-size-from-settings.md).
- `rustok-forum-storefront` pages both lists with the owner cursor. `ForumTopicPage` and
  `ForumReplyPage` carry `next_cursor` with `has_more` and `append_page`, so a fetched page extends
  the list the reader is looking at instead of replacing it, and ids already rendered are skipped
  because a keyset cursor can re-read a row a concurrent write moved into the next page. The next
  page continues on the variant the first page came from: unread-aware and audience-only topic
  cursors are produced by different owner services, so a principal that no longer holds
  `forum_topics:list` gets an error on the following page instead of a silently reordered feed.
  Both fetches omit `per_page`/`limit` so the owner applies the module settings, and both exist on
  both transports — the GraphQL list queries and the `forum/storefront-topic-page` /
  `forum/storefront-reply-page` `#[server]` endpoints - with no cross-transport fallback. The feed
  header count stays the selected category's **global** authored topic count (which may include
  topics this viewer cannot access) and the paging footer reports how many entries are actually loaded; the thread timeline measures the loaded window against the
  topic's `reply_count`, never a position inside posts that were not fetched. A failed page is
  named in the list that failed. Guarded by
  `scripts/verify/verify-forum-storefront-cursor-paging.mjs`.
- Topic title and post body length limits are enforced on topic create, topic update (title or body
  supplied), topic translation upsert, reply create, and reply update (content supplied). Imports and exact
  translation apply are not limited. The check counts characters of the trimmed title and of the plain text of
  the body; `0` as a maximum means unlimited. The admin form edits only the title limits; body limits are
  set through the settings JSON. See
  [`DECISIONS/2026-10-09-forum-content-length-enforcement.md`](../../../DECISIONS/2026-10-09-forum-content-length-enforcement.md).
- `max_edit_window_minutes` is enforced on author topic and reply updates, including standalone
  quote replacements through REST and GraphQL. Both transports inject the tenant settings providers
  into `ForumQuoteCommandService`; a settings read failure propagates before relation persistence.
  After the window closes, an author update returns `FORUM_VALIDATION_FAILED` (HTTP 400).
  Moderators and administrators are not limited. See
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
  (`Pending` to `Archived`). A pending topic cannot be promoted to a Blog post. The creation
  event is emitted on first publication (direct `Open` create, `Pending` to `Open`,
  or first-ever reopen of a rejected pending topic). A second archive/reopen
  never duplicates that event; the append-only Forum status journal is the
  publication-history source. Dedicated approve/reject transports remain unimplemented. See
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
- Reply notifications are delivered. `forum.topic.replied` is claimed by the host outbox
  intake decoder, described against the approved reply, and fanned out to topic-level
  watchers first and to the owning category's watchers second: an explicit topic-level row
  always wins (so `Normal` with replies off suppresses the category default), `Muted` rows
  and the reply author are never recipients, and each candidate is rechecked against that
  recipient's current topic and reply visibility before a notification is materialized,
  with a pending reply retried rather than dropped. Opening the notification re-authorizes
  the reply and deep-links `/modules/forum?category=…&topic=…&reply=…`. The intake selects
  rows from the host decoder's claimed types, so this needed no Notifications change. See
  [`DECISIONS/2026-10-11-notifications-outbox-intake-decoder-claims.md`](../../../DECISIONS/2026-10-11-notifications-outbox-intake-decoder-claims.md).
- Reply deep links address one post. `target_route` and the search projection emit
  `?reply=<reply-uuid>`, and the Next.js forum section and topic page validate that id, page
  forward through `forumStorefrontReplies` for at most 20 pages until the post is rendered, then
  scroll to it and mark it; a reply this reader cannot see, or one past that bound, is stated as
  absent from the loaded part of the thread instead of guessed at. The owner keeps no
  "page containing this reply" read, so exact placement inside a very long thread costs extra
  round trips, and the Leptos module widget has no anchor handling because no producer links to
  it. See
  [`DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md`](../../../DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md).
- Notification descriptors carry identifiers only, so the Notifications storefront inbox
  renders `forum.topic.replied` / `forum.topic.created` as the body and the notification
  type as the title until the localized template registry (`NOTIFY-06`) lands. The forum
  deliberately does not copy topic titles or reply text into the stored payload: reply
  bodies and restricted topic titles would be duplicated into per-recipient rows that
  outlive the visibility rules which currently allow them.
- Topic move (`FORUM-21A`) has an owner service but no transport or UI.
  Topic revision history has services but no API or UI.
- Attachment relations have a service but no transport or UI.
- Widget list previews (`forum.topic_list`) run under owner read semantics, which
  `DECISIONS/2026-10-09-forum-widget-preview-owner-audience.md` chose deliberately: the candidate
  query applies no storefront display settings, so a preview lists `closed` and `pending` topics
  that a viewer may read through the owner path but never in a storefront feed, and it ignores
  `show_locked_topics_in_lists` even though the storefront list honors it. The preview also checks
  the audience per candidate up to `FORUM_WIDGET_TOPIC_LIST_MAX_CANDIDATES` instead of resolving
  facts once per batch — the follow-up that ADR records — and a larger candidate set returns a
  validation error rather than a partial total. Page Builder gets a faithful owner read, not a
  pixel copy of the rendered block.
- Forum search documents carry the exact filter facts (`author`, `tags`, solved state,
  `published_at`, `channel_slugs`) and the Search module filters on them, but the
  indexed Search storefront surface does not yet render controls for author, tag, solved,
  locale, date or current-channel filtering. Forum's owner-backed feed selection below is
  separate from the Search index contract. The transports are additive and the owner
  plan tracks the missing product surface under `FORUM-23B2F1` to `FORUM-23B2F4`.
- Topic view counters are not implemented. Topic lists support owner-backed Latest
  (pinned first, then recent activity) and Top (canonical reply count descending,
  then the full Latest tie-break key). Search matches the displayed effective-locale
  title or slug, and Solved selects the canonical solution relation before output pagination.
  `last_reply_at` is a derived
  projection recomputed from the newest approved reply in both counter directions, so
  an imported or restored reply cannot move a topic by a timestamp the data does not
  carry; `updated_at` still advances with the counter write.
- `forumStorefrontUnreadTopics` defaults to the full authenticated, audience-visible
  feed with read-state badges for the native/GraphQL Leptos storefront. Its
  `unreadOnly: true` mode (used by the Next.js unread tab) selects only unread
  topics *after* audience and canonical read-state checks, scanning further
  bounded owner pages through read-only stretches. Its cursor belongs to the
  last returned unread topic, and is present only when another unread topic
  exists. Sparse unread feeds can scan many owner candidates; an indexed
  unread-and-audience composition and PostgreSQL performance evidence remain
  within the unfinished `FORUM-20` work.
- The Next.js storefront pages topic feeds and replies with the owner's `nextCursor`,
  appending deduplicated visible items; a full thread refresh invalidates any older
  in-flight reply page so it cannot roll back the cursor after posting. Reply deep links
  additionally scan a bounded number of pages (see
  `DECISIONS/2026-10-11-forum-reply-anchor-route-contract.md`). The category rail,
  overview and composer fetch all audience-visible categories in aligned 100-item
  GraphQL pages (up to the owner's 512-node bound) using `pageInfo.totalCount`;
  the overview also includes deeper descendants. A search match on a root
  category retains its complete visible branch, including siblings whose own
  names/slugs do not match. The composer requires an explicit category when opened from the unfiltered feed: it never silently
  assigns an unscoped draft to the first category. The chosen category owns both
  the topic mutation target and the current viewer's draft key; switching to a
  category with an existing saved draft restores that draft and saves the
  outgoing edits under their previous key rather than overwriting either.
  Closing or switching editors and leaving the route flush pending text before
  cancelling autosave; failed browser writes are reported in the editor and block
  a draft switch. Reply draft persistence additionally keys and validates the exact
  `(topicId, parentReplyId)`; top-level replies and each reply branch restore separately and
  successful cleanup removes only that target. Corrupt destination reads fail before retargeting
  the editor. Historical topic-only reply keys remain untouched but are not restored with an
  invented parent. Preview keeps the canonical discussion frame mounted in read-only mode,
  preserving nested richtext without duplicate quote blocks; public reads still use server
  projections. Both thread Retry controls repeat content/category reads and failed read receipts,
  with stale-response guards and no duplicate pending/successful receipt writes.
  Inline and full-page thread timelines track the actual post at the viewport reading line
  within their own DOM root. Jump to end follows the same audience-scoped opaque reply cursors,
  reading at most 20 additional pages per explicit click. If pages remain, a localized notice
  offers continuation by another click; it never labels the loaded frontier as the actual end.
  Snapshot refreshes and viewer/topic changes cancel stale walks, pending reads cannot duplicate,
  and the jump scroll runs after commit without replaying on later appends. A terminal cursor
  supplies the exact visible post total. The automatic `?reply=` anchor budget is unchanged.
  Search/Top/Solved now delegate to the server over the full authorized selection, including
  older candidate pages. Candidate reads stay bounded; sparse localized search/Solved can scan
  many server batches, not download them into the browser. Next debounces search and rejects
  late pages/errors from another selection. Native and GraphQL Leptos requests carry the same
  typed selection through initial/continuation reads; current Leptos controls still request Latest.
  Cursors bind tenant/viewer/category/status/locale/fallback/channel/surface and search/order/solved;
  mismatches and pre-cutover opaque cursors fail validation. Migration 39 adds tenant-first global
  and category Top indexes. This packet is source-only until Rust/runtime/plan verification;
  see [the owning decision](../../../DECISIONS/2026-10-17-forum-topic-feed-selection.md).
  The loaded category list clears an unavailable category selection after an
  account or audience change. Highlighting text in a thread opens a reply to
  that thread (or inserts into an already-open composer), with an exact quote
  relation only if its revision lookup succeeds. Next.js hides reply actions
  for locked or non-`open` topics; the write owner still decides permission.
- The requested remediation recheck also guards stale feed/thread refresh callbacks before they
  change request generations, keeps appended pages on whitespace-only search edits, and requires
  successful reply-draft restoration before quoting. Pending quote reads are cancelled on tooltip
  unmount. Next composer title/quote controls freeze during a write, with a synchronous duplicate
  submission lock and retry release. The owner plan records failing-before regressions and the
  remaining Rust/browser/performance verification limits.
- Continued draft inspection keeps a scoped composer open when the closing flush fails and
  validates saved documents through the shared richtext discussion profile (empty drafts allowed).
  Failed post-success draft deletion now shows an explicit cleanup-only retry. Session-local
  pending cleanup guards both direct restore and category switching against already-posted text;
  blocked storage still requires successful cleanup before reload/session remount. No durable
  idempotency or cross-tab recovery guarantee is introduced. The owner plan records four additional
  regressions and their 124-test scoped evidence, not mounted-browser/runtime acceptance.
- The next asynchronous-composer pass adds a composer-owned editing lifetime so closing and
  reopening the same target cannot revive old quote lookups. A synchronous context submission
  lock freezes target, title/document and quote mutations before the pending render; failed writes
  release it for retry. Late quote cleanup preserves newer text selections and always releases
  its in-flight lock. The owner plan records 12 additional tests and their 136-test scoped
  evidence, still without mounted React/browser or Rust/runtime acceptance.
- Frame replacement follows the same composer-owned lifetime: its render projection keys a
  distinct richtext frame and document callbacks reject replaced sessions. Shared richtext
  controllers also ignore queued messages/handshakes after disposal; Forum does not fork the
  protocol. Six further checks bring current evidence to 139 Forum/ownership plus 13 richtext
  tests (152 combined at that pass). The owner plans retain browser/runtime limitations.
- The dependency follow-up aligns Tiptap, React/render/types and host lint/compiler peers;
  native host typecheck/lint now pass without the previous scratch/forced-install workarounds.
  That pass records 156 Forum/ownership/richtext tests. Host build error suppression
  is removed; admin Docker/npm/asset wiring is updated but not image-runtime verified here.
- Form submission now checks its captured editing lifetime before validation or locking: a
  retained callback from a closed/replaced/successfully cleared composer cannot publish its old
  document again. Composer headings and the quotes label are catalog-owned in English/Russian.
  Five further checks bring scoped evidence to 161 combined tests (120 storefront / 12 admin
  through host scripts), without claiming browser or server-idempotency acceptance.
- The Next-admin reply composer clears its document only after a successful owner write,
  retains the submitted content locale, and preserves failed or genuinely newer input.
  Topic/locale/tenant/auth changes start a new editor session; late old-session completions
  cannot clear it or show stale notices. Pending writes reject duplicate submits.
- The Leptos storefront composer writes real content: topics go through
  `createForumTopicWithQuotes` (GraphQL) or `forum/storefront-topic-create`
  (native `#[server]`) and replies through `createForumReplyWithQuotes` or
  `forum/storefront-reply-create`, both composed onto the owner create commands
  with the transport pinned in the audience context. The build-profile-selected
  transport is never swapped on failure. The composer posts plain text into the
  canonical discussion document only: tags, explicit slug, attachments, edit,
  and inline quote-of-quote UI stay unimplemented, and the on-screen failure copy is
  the redacted transport message because `ApiError`/`TransportError` display does
  not leak owner reasons (`locked`, `cooldown`, audience denial) to an
  untrusted surface. Reply and quote affordances are hidden for a locked or
  non-`Open` topic from the topic detail the read path already returns; the owner
  check stays authoritative. Composer quotes are forwarded as owner relations only
  when an exact positive revision is known; the storefront reply read model carries
  no revision and the module UI must not look one up per visible reply, so a
  Leptos reply quote posts quoted text without a pinned relation. A pinned quote
  needs a caller-provided revision, as the Next.js composer does.
Only part of the module's guard family runs automatically: 13 of the 259
  `scripts/verify/verify-forum-*.mjs` gates are wired into 8 workflows, the rest
  execute only when someone invokes them, so a red one is not by itself evidence
  about the tree and a green one is not evidence about a rule. `node
  scripts/audit/verifier_status.mjs --prefix forum-` classifies the whole family
  in one pass; measured on the 2026-10-11 tree that is 88 pass, 170 exiting with
  their own structured finding, 1 needing `cargo`, and none throwing before
  evaluating a rule. The throwing class is gone rather than re-labelled: three
  gates died on their own harness or inputs.
  `verify-forum-owner-boundary.test.mjs` built a fixture that omitted
  `services/category_owner.rs`, which the guard it exercises requires, and the
  attachment admission guard read `src/category_presentation.rs`, a file the
  canonical-category cutover deleted; the first is green across all six cases, the
  second now asserts the retired path stays removed, fails when an input is
  missing, and checks production code only - it had also banned `Uuid::new_v4`
  across the whole file, which only its `#[cfg(test)]` module uses. The audit tool
  itself had been over-reporting: a guard that signals a violation with `throw new
  Error(...)` is a verdict, not a crash. Four wired gates pinned the mid-cutover
  source shape instead of the boundary - the read-model guard demanded the retired
  `src/services/read_model.rs` delegate and that the owner query
  `forum_category_taxonomy_binding` directly, and the two taxonomy guards required
  the `ForumCategory` hierarchy columns that
  `m20260916_000032_clean_forum_category_canonical_taxonomy` dropped. They now
  assert the current contract (category copy only through
  `TaxonomyOwnerCategoryReader`, locale fallback only through the shared Content
  helper, `enforce_scope` on every read entry point, and the retired delegate,
  binding entity and columns staying retired), so a failure means a boundary moved
  rather than that history was rewritten. Two audience guards failed for the same
  reason: the topic and category audience policies had been split into a helper
  module plus a thin owner seam composed with `include!`, so a rule that isolated
  `pub async fn set(` by slicing to the next sibling name read the wrong text,
  `lock_topic_audience_in_tx` was a marker that never existed (the seam takes
  `lock_active_topic_audience_write_in_tx` and
  `lock_topic_audience_scopes_in_tx`), and an unanchored `channel::` ban matched
  Forum's own `forum_category_audience_channel::Entity`. They now isolate
  functions by matching braces, accept a rule satisfied in either file, and anchor
  foreign-module paths; the topic gate is green and the category gate is left red
  on its canonical-plan markers. One finding was real: `services/mod.rs`
  re-exported `ForumTopicRouteTombstoneVisibilityService` straight from the
  internal `mod topic_owner`, against the convention the same file uses for
  owner-backed services, so that service now comes through a named seam module and
  the public path consumers use is unchanged, which is what flipped both
  owner-layout guards green. The topic-route tombstone guard had also pinned the
  composition spelling itself (`include!(...)`) while
  `services/topic_owner_inline.rs` wires that file as `#[path = ..] pub mod
  route_tombstone_visibility;`; both spellings keep the implementation inside the
  internal owner module, so it accepts either. The audience host-runtime guards
  (`FORUM-20A-*`) now fail only on canonical-plan markers too:
  `docs/implementation-plan.md` records FORUM-20 once and has neither the `##
  FORUM-20` visibility-boundary section nor the `FORUM-20A-G` / `FORUM-20A-AU
  provide` rows or the `## FORUM-26` heading, while their code pins pass - so the
  adapter propagation is verified and writing those plan rows belongs to the
  packets that own them, not to a silent documentation edit; the same
  delivered-evidence pins account for 21 further gates whose only failures are
  markers of delivered slices the plan does not carry and that no contract claims.
  Two more guards were the same class of defect. The FORUM-33G reconciliation
  guard pinned `impl NotificationInboxReconciliationInspectPortFactory for ...`
  and a receiver chain that rustfmt had already wrapped, so it reported a missing
  implementation that the module has; it now compares markers on a
  layout-normalized copy and is green. The approved-posts guard read one file of
  an owner module that `services/mod.rs` composes from four `include!` siblings -
  the moderation decisions live in `reply_owner_inline.rs` and `topic_inline.rs` -
  and pinned `status: Set(TopicStatus::Open)`, which the accepted topic
  pre-moderation decision had superseded, plus a `query_one` call the adapter
  issues as `query_one_raw`; it now reads the composition through
  `services/mod.rs`, cites the ADR rule, and reports no code finding at all,
  leaving only the canonical-plan markers. The category slug alias guard combined
  both defects and one contradiction: it pinned `DROP TABLE
  forum_category_route_aliases` while the alias-history test drops that table as
  `DROP TABLE IF EXISTS ..`, and it required `category_route.rs` to reference
  `forum_category_taxonomy_binding` - a runtime read of the binding table that the
  same module's taxonomy guard asserts is retired, so the two guards demanded
  opposite shapes and only the port-based route could satisfy both; the marker is
  now a layout-tolerant pattern and the binding reference is a forbidden pattern.
  Both probes bite. the completed cutover deleted (`rustfmt --check
  crates/modules/rustok-forum/src/services/read_model.rs` in
  `forum-read-model-category-taxonomy-copy.yml`, and `cargo test -p rustok-forum
  --test category_taxonomy_binding`, a target that does not exist, in
  `forum-taxonomy-category-binding.yml`); neither can pass, and retiring them is a
  CI-owner change that must not be made from a feature branch.

## Roadmap

[`docs/implementation-plan.md`](./docs/implementation-plan.md) is the only
authoritative forum roadmap and task-status source. Do not copy its task ledger
into README files, issues, or additional planning documents.

## Docs

- [CRATE_API](./CRATE_API.md)
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
