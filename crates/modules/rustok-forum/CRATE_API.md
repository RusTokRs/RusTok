# rustok-forum / CRATE_API

## Public Modules
`category_presentation`, `constants`, `controllers`, `dto`, `entities`, `error`, `graphql`, `locale`, `mentions`, `services`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `ForumModule` implementing `RusToKModule` (slug `"forum"`, dependencies `["content", "media", "taxonomy"]`) and `MigrationSource`.
- **Domain Services**:
  - `CategoryService`: Hierarchy trees, move/reorder sibling mutations, subtree archiving/restoration, topic policy, audience policies.
  - `TopicService`: Topic creation, updates, lifecycle (open/close/archive/lock/pin), solution assignment, and feed ranking (Latest/Top/Solved).
  - `ReplyService`: Discussion replies, threading, branch splits/forks/merges, and moderation status transitions.
  - `ModerationService`: Forum moderation actions and audience-authorized administrative operations.
  - `SubscriptionService` & `UserStatsService` & `VoteService`: Forum engagement, subscription digest preferences, user statistics, and upvote/downvote operations.
  - `ForumAttachmentRelationService`: Media attachment relations with CAS revision fencing and hold reconciliation.
- **Runtime Extensions & Capabilities**:
  - Registers `ForumSearchProjectionSourceFactory` via `rustok-core`.
  - Registers `ForumCategorySeoTargetProvider` and `ForumTopicSeoTargetProvider` via `rustok-seo-targets`.
  - Registers `ForumReactionSubjectProviderFactory` via `rustok-reactions`.
  - Registers `ForumModerationSubjectAdapterFactory` (topic and reply) via `rustok-moderation-api`.
  - Registers `ForumNotificationSourceProviderFactory` via `rustok-notifications-api`.

## Domain Collaborations

- **`rustok-content`**: Node-based backing storage for forum content items (`forum_category`, `forum_topic`, `forum_reply`).
- **`rustok-media`**: Retention and release of media asset references associated with forum attachments.
- **Taxonomy owner via `ForumTaxonomyPort`**: Shared category route descriptors, localization projections, hierarchy and topic tag associations. `rustok-taxonomy` is a test-only Cargo dependency; the joint Forum+Taxonomy server distribution registers an in-process owner adapter before consumer snapshots; Rust/runtime verification remains pending.
- **`rustok-reactions`**: Pluggable engagement provider based on the `forum.useReactions` setting.
- **`rustok-moderation`**: Subject adapter integration for automated and manual topic/reply moderation.
- **`rustok-notifications`**: Domain event notifications for user mentions and subscription digests.
- **`rustok-outbox`**: Transactional outbox event publishing for forum events.

## Topic feed selection

`ListTopicsFilter` adds optional `search`, typed `ForumTopicListOrder` (`latest` default,
`top`) and `solved_only` (default false). REST uses those snake_case names; GraphQL
`forumTopics`, `forumStorefrontAudienceTopics` and `forumStorefrontUnreadTopics` expose
`search`, `order: ForumTopicListOrder` (`LATEST`/`TOP`) and `solvedOnly`. Existing auth,
tenant, audience, locale/fallback, channel and page-size contracts still apply. Unread
composition remains `unreadOnly`, not a browser filter.

Search is a trimmed Unicode-lowercase literal substring of the owner's displayed title/slug,
with a 200-character normalized budget. Solved means `solution_reply_id` is present. Top
uses stored canonical `reply_count DESC` before the complete Latest tie-break key. Localized
selection refills from bounded candidate pages; a sparse match can require many server batches.
No total count or multi-request snapshot is added. Database/policy failures propagate.

Topic cursors include rank and a context/selection fingerprint. Changing selection/context
requires a fresh first page; changing page size does not. Old opaque encodings are rejected
without a legacy reader. The independent `ForumReadModelService` uses unbound Latest cursors
and rejects scoped/ranked feed cursors. Migration 39 adds global/category Top indexes; PostgreSQL
runtime/EXPLAIN and Rust/native feature verification remain pending in this environment.
See [the decision](../../../DECISIONS/2026-10-17-forum-topic-feed-selection.md).

## Primary Public Types and Signatures
- `pub struct ForumModule`
- `pub struct CategoryService`, `TopicService`, `ReplyService`, `ModerationService`, `SubscriptionService`, `UserStatsService`, `VoteService`
- `pub struct ForumAttachmentRelationService`
- `pub struct ForumAttachmentHoldReconciliationService` — read-only Media hold reconciliation report with bounded keyset pagination
- `ForumAttachmentRelationService::get_attachment_relations(tenant_id, target, locale) -> ForumAttachmentRelationSet`
- `ForumAttachmentRelationService::replace_attachment_relations(context, ForumAttachmentRelationAdmissionRequest) -> ForumAttachmentRelationSet`
- `pub struct ForumRelationReadService`
- `ForumRelationReadService::get(tenant_id, security, ForumRelationSnapshotQuery) -> ForumRelationSnapshotResponse`
- `pub struct ForumQuoteCommandService`
- `ForumQuoteCommandService::with_settings_providers(ForumSettingsProviders) -> Self` — host-injected
  tenant policy; standalone quote writes enforce the same author edit window as inline updates.
- `ForumQuoteCommandService::set_topic_quotes(tenant_id, topic_id, security, SetForumQuotesInput) -> ForumRelationSnapshotResponse`
- `ForumQuoteCommandService::set_reply_quotes(tenant_id, reply_id, security, SetForumQuotesInput) -> ForumRelationSnapshotResponse`
- `TopicService::create_command(tenant_id, security, CreateTopicCommandInput) -> TopicResponse`
- `TopicService::create_with_audience_context(tenant_id, security, PortContext, CreateTopicInput) -> TopicResponse`
- `TopicService::create_command_with_audience_context(tenant_id, security, PortContext, CreateTopicCommandInput) -> TopicResponse`
- `TopicService::with_audience_facts(db, event_bus, SharedForumAudienceFactsPort) -> TopicService`
- `pub struct ForumTopicCreateAudienceAuthorizationService`, `ForumTopicCreateAudienceAuthorization`
- `pub struct graphql::ForumGraphqlRuntimeData`; `graphql::attach_schema_data(GraphqlRuntimeInputs)`
- `TopicService::update_command(tenant_id, topic_id, security, UpdateTopicCommandInput) -> TopicResponse`
- `ReplyService::create_command(tenant_id, security, topic_id, CreateReplyCommandInput) -> ReplyResponse`
- `ReplyService::create_with_audience_context(tenant_id, security, topic_id, PortContext, CreateReplyInput) -> ReplyResponse`
- `ReplyService::create_command_with_audience_context(tenant_id, security, topic_id, PortContext, CreateReplyCommandInput) -> ReplyResponse`
- `ReplyService::with_audience_facts(db, event_bus, SharedForumAudienceFactsPort) -> ReplyService`
- `pub struct ForumReplyCreateAudienceAuthorizationService`, `ForumReplyCreateAudienceAuthorization`
- `ReplyService::update_command(tenant_id, reply_id, security, UpdateReplyCommandInput) -> ReplyResponse`
- `CategoryService::tree(tenant_id, security, CategoryTreeQuery) -> CategoryTreeResponse`
- `CategoryService::move_category(tenant_id, category_id, security, MoveCategoryInput) -> MoveCategoryResponse`
- `CategoryService::reorder_siblings(tenant_id, security, ReorderCategorySiblingsInput) -> ReorderCategorySiblingsResponse`
- `CategoryService::archive_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::restore_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::topic_policy(tenant_id, category_id, security) -> CategoryTopicPolicyResponse`
- `CategoryService::set_topic_policy(tenant_id, category_id, security, UpdateCategoryTopicPolicyInput) -> CategoryTopicPolicyResponse`
- `pub struct ForumCategoryTopicCreateAudiencePolicyService`
- `ForumCategoryTopicCreateAudiencePolicyService::get(tenant_id, category_id, security) -> ForumCategoryTopicCreateAudiencePolicy`
- `ForumCategoryTopicCreateAudiencePolicyService::set(tenant_id, category_id, security, SetForumCategoryTopicCreateAudiencePolicyInput) -> ForumCategoryTopicCreateAudiencePolicy`
- `pub struct ForumCategoryReplyCreateAudiencePolicyService`
- `ForumCategoryReplyCreateAudiencePolicyService::get(tenant_id, category_id, security) -> ForumCategoryReplyCreateAudiencePolicy`
- `ForumCategoryReplyCreateAudiencePolicyService::set(tenant_id, category_id, security, SetForumCategoryReplyCreateAudiencePolicyInput) -> ForumCategoryReplyCreateAudiencePolicy`
- `pub struct ForumTopicReplyCreateAudiencePolicyService`
- `ForumTopicReplyCreateAudiencePolicyService::get(tenant_id, topic_id, security) -> ForumTopicReplyCreateAudiencePolicy`
- `ForumTopicReplyCreateAudiencePolicyService::set(tenant_id, topic_id, security, SetForumTopicReplyCreateAudiencePolicyInput) -> ForumTopicReplyCreateAudiencePolicy`
- `pub struct ForumCategoryModerationAudiencePolicyService`
- `ForumCategoryModerationAudiencePolicyService::get(tenant_id, category_id, security) -> ForumCategoryModerationAudiencePolicy`
- `ForumCategoryModerationAudiencePolicyService::set(tenant_id, category_id, security, SetForumCategoryModerationAudiencePolicyInput) -> ForumCategoryModerationAudiencePolicy`
- `pub struct ForumModerationAudienceAuthorizationService`, `ForumModerationAudienceAuthorization`
- `ModerationService::with_audience_facts(db, event_bus, SharedForumAudienceFactsPort) -> ModerationService`
- `ModerationService::*_with_audience_context(..., PortContext) -> ForumResult<()>` for reply status, topic pin/lock/status, and solution commands
- `TaxonomyOwnerCategory`, Taxonomy-backed Category owner mutation adapters and Media owner-reference retention
- `extract_forum_mention_candidates(document: &RichTextDocument, policy) -> ForumResult<ForumMentionCandidates>`
- `resolve_forum_mentions(profiles, tenant_id, candidates, requested_locale, tenant_default_locale) -> ForumResult<ForumResolvedMentions>`
- `validate_forum_quote_references(source, references) -> ForumResult<Vec<ForumQuoteReference>>`
- `diff_forum_mentions(previous, current) -> ForumResult<ForumMentionDiff>`
- `ForumRevisionIdentity`, `ForumMentionRevisionProjection`, `ForumMentionEventCandidate`, `ForumQuoteReference`
- `ForumQuoteTargetKindInput`, `ForumQuoteReferenceInput`, `SetForumQuotesInput`
- `CreateTopicCommandInput`, `UpdateTopicCommandInput`, `CreateReplyCommandInput`, `UpdateReplyCommandInput`
- `ForumRelationSnapshotQuery`, `ForumRelationSnapshotResponse`, `ForumRelationQuoteResponse`
- `pub mod graphql` -> `ForumQuery`, `ForumMutation`
- GraphQL reconciliation entry point: `forumAttachmentHoldReconciliationReport(limit, mediaAfter, relationAfter)`; operator-only, page-local, bidirectional read-only Media hold/relation audit
- `pub mod controllers` -> `routes()`
- Public DTOs/constants from `dto::*` and `constants::*`
- `pub enum ForumError`, `pub type ForumResult<T>`
- `pub mod locale` — helpers `resolve_translation`, `resolve_body`, `available_locales`

## DTO changes (current)
### TopicResponse
- Canonical content: `body: RichTextView`, `body_plain_text: String`.
- Locale and state: `requested_locale`, `effective_locale`, `available_locales`, `slug`, `author_id`, votes, subscription state, and `solution_reply_id`.
### TopicListItem
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `slug: String`, `author_id: Option<Uuid>`, `vote_score: i32`, `current_user_vote: Option<i32>`, `is_subscribed: bool`, `solution_reply_id: Option<Uuid>`
### ReplyResponse / ReplyListItem
- `ReplyResponse` exposes `content: RichTextView` and `content_plain_text: String`.
- `ReplyListItem` exposes only the bounded server-derived plain-text preview required by list surfaces.
- Locale, author, parent, vote, and solution state remain explicit.
- Both reply surfaces return the shared multilingual contract fields
  `requested_locale`, `effective_locale`, and `available_locales` computed
  from the stored reply bodies, matching the topic/category/tree/read-model
  surfaces. `GqlForumReply` mirrors the same fields.
### CategoryResponse
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `is_subscribed: bool`
### CategoryListItem
- Added: `parent_id: Option<Uuid>`, `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `is_subscribed: bool`
### Category tree
- Added: `CategoryTreeQuery`, `CategoryBreadcrumb`, `CategoryTreeNode`, `CategoryTreeResponse`.
- The canonical tree returns the complete tenant hierarchy in deterministic `(position, id)` sibling order through one owner call bounded to 512 nodes and zero-based depth 16.
- Each node includes `parent_id`, `depth`, direct `children_count`, `has_children`, localized breadcrumbs, `allows_topics`, `archived_at`, `is_archived` and nested children.
- REST entry point: `GET /api/forum/categories/tree`.
- GraphQL entry point: `forumCategoryTree(tenantId, locale, fallbackLocale)` on the merged `ForumQuery`.
- Categories without any localized translation fail closed instead of returning empty `name`/`slug` fields.
- The legacy flat category list remains a bounded compatibility projection and is not the canonical hierarchy contract.
### Category placement commands
- Added: `MoveCategoryInput`, `ReorderCategorySiblingsInput`, `CategoryPlacementResponse`, `MoveCategoryResponse`, `ReorderCategorySiblingsResponse`.
- `move_category` serializes hierarchy mutations per tenant, rejects cycles, foreign parents and depth overflow, and normalizes source and destination sibling positions in one transaction.
- `reorder_siblings` requires the complete direct-child set exactly once and persists contiguous zero-based positions atomically.
- REST entry points: `PUT /api/forum/categories/{id}/move` and `PUT /api/forum/categories/reorder`.
- PostgreSQL and SQLite reject category writes whose resulting zero-based depth would exceed 16, including internal direct writes that bypass owner services.
- Generic `CategoryService::update` rejects `position`; placement changes must use `move_category` or `reorder_siblings`.
### Category subtree lifecycle
- Added: `CategorySubtreeLifecycleResponse`.
- Absence of a lifecycle row means active, preserving existing categories without backfill.
- `archive_subtree` writes descendants before ancestors; `restore_subtree` removes ancestor lifecycle rows before descendants under the same tenant category-tree lock.
- REST entry points: `POST /api/forum/categories/{id}/archive-subtree` and `POST /api/forum/categories/{id}/restore-subtree`.
- GraphQL entry points: `archiveForumCategorySubtree` and `restoreForumCategorySubtree`.
- Existing topics are preserved. Archived categories reject new topic placement and active children; a subtree cannot be partially restored beneath an archived ancestor.
### Category topic policy
- Added: `UpdateCategoryTopicPolicyInput` and `CategoryTopicPolicyResponse`.
- Absence of a stored row means `allows_topics = true`, preserving behavior for existing categories.
- REST entry point: `GET/PUT /api/forum/categories/{id}/topic-policy`.
- GraphQL entry points: `forumCategoryTopicPolicy` and `setForumCategoryTopicPolicy`.
- PostgreSQL and SQLite reject direct `forum_topics` inserts or category moves into a category whose policy disables topic creation.
- Existing topics remain unchanged when a category policy is disabled; the policy controls new topic placement only.
### Category topic-create audience policy
- `ForumCategoryTopicCreateAudiencePolicyService` stores a separate normalized topic-create audience rule; it does not mutate content visibility.
- Effective topic-create audience is the root-to-category conjunction of every non-empty local layer.
- Managed `get` and atomic replacement `set` require `forum_categories:manage`; empty constraints restore inheritance.
- PostgreSQL and SQLite enforce tenant/category ownership, typed relations, immutable rows, and bounded direct channel/group/user inserts.
- `FORUM-20AQ` publishes normalized persistence; `FORUM-20AR` composes it into every public topic-create owner method.
- Categories without a topic-create layer retain historical behavior; local role and explicit-user decisions require no owner facts.
- Unresolved trust, Channel, or Groups selectors require an exact caller `PortContext` and an injected `SharedForumAudienceFactsPort`; missing composition fails closed.
- Authorization runs before topic, translation, relation, counter, user-stat, and domain-event writes and returns one generic public denial.
- `FORUM-20AS` composes GraphQL and REST topic-create calls through exact authenticated `PortContext` values and consumes the host-published optional facts port without adding identity to DTOs.
- Both legacy and inline-quote topic-create transports preserve the owner gate before writes; categories with local decisions remain compatible when the provider is absent.
- The Forum manifest publishes `graphql::attach_schema_data`, while the HTTP router consumes the same `SharedForumAudienceFactsPort` from `HostRuntimeContext`.
- `topic_create_audience_port_context` is a public owner seam, and `ForumTopicCreateTransport` carries `Graphql`, `Rest`, and `NativeServer`. A module-owned Leptos storefront SSR handler passes `NativeServer` so its write keeps the same pinned transport fact as GraphQL and REST instead of re-implementing the gate; an unknown or mismatched transport label fails closed.
- Run `node scripts/verify/verify-forum-category-topic-create-audience-policy.mjs`, `node scripts/verify/verify-forum-topic-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs` after changing this boundary.
### Category reply-create audience policy
- `ForumCategoryReplyCreateAudiencePolicyService` stores a separate normalized reply-create audience policy; it does not mutate category/topic visibility or the topic-create policy.
- Effective reply-create audience is the root-to-category conjunction of every non-empty local layer.
- Managed `get` and atomic replacement `set` require `forum_categories:manage`; empty constraints restore inheritance.
- PostgreSQL and SQLite enforce tenant/category ownership, typed roles/trust/effects, immutable rows, and bounded direct channel/group/allow/deny inserts.
- `FORUM-20AU` publishes normalized owner persistence; `FORUM-20AV` composes the inherited policy into every public reply-create owner method before any raw owner write.
- Categories without a reply-create layer retain historical behavior; local role and explicit-user decisions require no owner facts.
- Unresolved trust, Channel, or Groups selectors require an exact caller `PortContext` and an injected `SharedForumAudienceFactsPort`; missing composition fails closed.
- Authorization resolves the tenant-scoped topic/category identity and runs before reply, body, relation, counter, user-stat, and domain-event writes with one generic public denial.
- `FORUM-20AW` composes GraphQL and REST reply-create calls through exact authenticated `PortContext` values and reuses the host-published optional facts port without adding identity to DTOs.
- Both legacy and inline-quote GraphQL and REST reply-create transports preserve the owner gate before writes; locally decidable policies remain compatible when the provider is absent.
- `ForumGraphqlRuntimeData` and `ForumHttpRuntime` create `ReplyService` from the same optional `SharedForumAudienceFactsPort` already used by topic creation.
- `reply_create_audience_port_context` is a public owner seam, and `ForumReplyCreateTransport` carries `Graphql`, `Rest`, and `NativeServer`, so the module-owned Leptos storefront SSR handler pins its transport label the same way topic creation does.
- Run `node scripts/verify/verify-forum-category-reply-create-audience-policy.mjs`, `node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs` after changing this boundary.
### Topic reply-create audience narrowing
- `ForumTopicReplyCreateAudiencePolicyService` stores an optional normalized topic-local reply-create layer independently from `forum_topic_audience_*` content visibility.
- Managed `get` and atomic replacement `set` require `forum_topics:manage`; empty constraints clear only the local topic layer.
- `FORUM-20AX` evaluates all inherited category layers followed by the optional topic layer, so a topic can narrow but never broaden its category reply-create policy.
- Internal authorization identifies the exact category or topic layer that denied the actor while retaining one generic public denial.
- Existing legacy and inline-quote owner paths, plus their composed GraphQL and REST transports, inherit the topic check without DTO or transport changes.
- PostgreSQL and SQLite enforce tenant/topic ownership, typed relations, immutable rows, and bounded direct channel/group/allow/deny inserts.
- Run `node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs`, `node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs` after changing this boundary.
### Selected hierarchy for create admission

Category topic/reply-create policy owners, the topic-local reply-create policy
owner and both create-authorization services accept `with_taxonomy_port`. They
use the selected provider for inherited layers and retain the actual connection;
managed policy reads/replacements and command admission retain their transaction.
Every public topic/reply create path uses its configured audience owner even when `PortContext` is absent.
Local selectors still need no facts capability; unresolved external selectors
still fail closed without exact context/facts. A topic can narrow but never broaden
its category reply-create policy. Provider failure is not an unrestricted policy.
GraphQL/REST/native commands reuse the existing configured facade; no DTO or
client schema changes are introduced by this wiring.

Rust tests remain uncompiled/unexecuted because Cargo is absent. Source checks
and WASM formatting are not runtime evidence. The Forum-only
`taxonomy_read_wiring` suite includes authored inheritance, provider failure,
transaction retention and rollback regressions for this boundary. Structural command
and tombstone continuation evidence and remaining runtime gates are tracked in `docs/implementation-plan.md`.

### Audience authorization and ORM relation invariants

- `evaluate*` methods may return a denied authorization for previews. Both pool
  `require` and crate-private transactional `require_in_tx` must instead return
  `ForumError::Forbidden` on denial. They share one denial conversion per owner;
  raw topic/reply writers must not need a second `allowed` check to enforce it.
- All 24 normalized category/topic audience role/channel/group/user entity
  relations use `(TenantId, CategoryId/TopicId)` on **both** sides of the policy
  FK. Production migrations already use those composite keys; no migration is
  introduced. Single-column ORM relations generate invalid SQLite FKs against a
  composite primary key and do not express tenant-scoped ORM joins.
- `tests/audience_relation_schema.rs` contains six authored SeaORM schema/FK/
  cascade regressions with foreign keys enabled. Two private
  `create_audience_authorization_tests` regressions directly exercise transactional
  require denial, rollback recovery and topic-local reply narrowing. All remain
  uncompiled/unexecuted. `tests/support/audience_schema.rs` is test-only.
- `npm run verify:forum:audience-relation-schema` checks declarations;
  `npm run test:verify:forum:audience-relation-schema` adds 74 guard cases, including
  SQLite execution of **declaration-projected** FK tuples and reproduction of the
  old mismatch. This is not execution of SeaORM-generated DDL or domain services.

### Selected hierarchy for structural commands

`ForumTopicSplitService`, `ForumTopicForkService`, `ForumReplyRangeMoveService`,
`ForumTopicMoveService` and `ForumTopicMergeService` accept `with_taxonomy_port`.
Split/fork/range source/target
visibility and reply-create comparisons use the selected hierarchy inside the
writer transaction; the GraphQL adapters forward the manifest runtime provider.
Reply-range moves also compare inherited public/authenticated base visibility,
not just richer topic/category layers. Provider failure aborts the command.
Standalone operation uses the explicit default Noop service capability; there is
no separate standalone reply-create projection implementation.

Split and range movement preserve source reply identities where applicable;
fork source reply identities remain unchanged and its new copies preserve original quote targets.
The fork target starts unsolved and does not copy votes/subscriptions/read state.
Move/merge validate the target's inherited policy with the selected provider inside
the writer transaction. They intentionally may change effective audience: unlike
reply-range movement, they do not require source/target audience equality. Both
GraphQL merge commands forward the selected runtime capability. Exact receipt
replay still returns the original result before current hierarchy/policy checks.

All structural placement and topic/reply restore gates use the same selected
ancestor lifecycle loader. It reads only parent IDs, validates complete tenant
membership/graph bounds and preserves provider errors. Topic creation and category
create/move parent validation use it too. The public reply facade forwards the
provider into its restore owner. Structural commands take the shared category-tree
scope before topic/counter row locks, consistently with delete/restore; replay is
kept ahead of those current-state gates.

Deletion captures anonymous base visibility and richer inherited audience in the
writer transaction through the selected provider. Existing immutable tombstone
history is not recomputed or rewritten. Provider errors abort the new snapshot,
not become public disclosure. The read-side boolean service needs no hierarchy
capability because it reads that sealed history only.

Six private `structural_taxonomy_tests` regressions cover selected lifecycle,
uncommitted ancestor/rollback recovery, invalid projection/provider failure,
post-write move/merge rollback and tombstone base/richer disclosure/failure.
Rust tests remain uncompiled/unexecuted; source guards cannot certify rollback,
concurrency, or mounted transport behavior. Broader consumer/host acceptance
remains mandatory in the canonical plan.

### Category moderation audience policy
- `ForumCategoryModerationAudiencePolicyService` stores a normalized category-local moderator audience independently from content visibility and posting eligibility.
- `FORUM-20AY` evaluates every inherited root-to-category layer before topic/reply moderation writes; explicit deny retains precedence and a child cannot broaden an ancestor.
- Managed `get` and atomic replacement `set` require `forum_categories:manage`; empty constraints restore inheritance.
- Existing context-free moderation methods preserve unrestricted, role-only, and explicit-user behavior. Trust, Channel, or Groups selectors require exact context plus `SharedForumAudienceFactsPort` and otherwise fail closed.
- `ModerationService::with_audience_facts` and context-aware command variants cover reply status, topic pin/lock/status, and solution commands. Admission runs **inside** the writer transaction after the category-tree fence and before domain effects; a pre-transaction preview is not a write authorization.
- Topic authors retain the existing owned-update path for marking or clearing solutions; only the moderator fallback is audience-gated.
- PostgreSQL and SQLite enforce tenant/category ownership, immutable rows, bounded relations, and one shared advisory key for owner replacement and direct bounded inserts.
- GraphQL/REST exact context composition remains `FORUM-20AZ`; no moderation DTO changed in `FORUM-20AY`.
- `ForumCategoryAudiencePolicyService`, `ForumCategoryModerationAudiencePolicyService`, `ForumModerationAudienceAuthorizationService` and `ModerationService` expose `with_taxonomy_port(Arc<dyn ForumTaxonomyPort>)`. Managed reads/replacements and moderation admission retain the selected hierarchy and actual transaction. GraphQL/HTTP factories forward the host selection, with or without audience facts. Standalone Noop is used only when deliberately unconfigured; a selected provider failure propagates without fallback.
- `tests/support/taxonomy_moderation_authorization.rs`, mounted by `taxonomy_read_wiring`, contains six authored, unexecuted regressions: richer/moderation local clearing, post-replacement provider failure/rollback, ancestor denial and positive authorization, uncommitted transaction policy, denied owner effects, and configured GraphQL/mounted HTTP composition. No UI/DTO/migration change is required. The trusted host `moderation_subject` adapter is a distinct workflow, not this public moderation audience owner.
- Run `node scripts/verify/verify-forum-moderation-audience-policy.mjs` after changing this boundary.
### Category presentation ownership
- Canonical Category identity, localized copy, hierarchy, routes, aliases and presentation are owned by `rustok-taxonomy`.
- Forum Category commands pass `icon` and `color` through the Taxonomy owner-sync boundary; Forum does not normalize or persist a second canonical presentation model.
- Forum Category reads consume provider-owned `ForumTaxonomyCategorySnapshot` values through `ForumTaxonomyPort`, including locale, hierarchy and presentation fields. The external adapter may map `TaxonomyOwnerCategory`; Forum does not import that persistence owner's DTO at this boundary.
- `ForumCategoryAudienceReadService::with_taxonomy_port`, `ForumCategoryAudienceVisibilityService::with_taxonomy_port`, `ForumCategoryVisibilityPolicyService::with_taxonomy_port`, `ForumPublicDiscoveryService::with_taxonomy_port` and `ForumSearchCategoryAudienceScopeService::with_taxonomy_port` preserve an injected provider. Category metadata and inherited base/richer audience layers use the same port; connection/transaction reads pass the caller's borrowed connection. `CategoryService::with_taxonomy_port` also forwards it to its base visibility service.
- GraphQL runtime composes `category_route_service` and category audience reads with the host port; REST selected/list/tree reads and native category data/route reads forward it too. Search projection routes and discovery share the source's injected port, while server Search subtree expansion forwards the extension binding. SEO category mapping/routes/discovery resolve the port from `SeoTargetRuntimeContext.runtime_extensions`. A configured provider failure propagates; it is not replaced with Noop or an empty successful route.
- Noop remains the explicit degraded standalone fallback and returns no canonical category route. The server implements `ServerForumTaxonomyPort` through Taxonomy owner APIs and registers it only for a joint Forum+Taxonomy distribution, before capability snapshots. `Connection`/`Transaction` are preserved and `None` returns `FORUM_TAXONOMY_CONNECTION_REQUIRED`; the adapter never silently falls back to its pool. Strict module-scoped parents and metadata do not synthesize missing placements. Owned alias history, hierarchy commands and tag writes delegate on the same transaction. Consumer/source evidence and authored real-owner tests still do not establish production taxonomy routing. Run `npm run verify:forum:taxonomy-wiring` / `npm run test:verify:forum:taxonomy-wiring`; `cargo test -p rustok-forum --test taxonomy_read_wiring` remains required.
- Whole-tenant parent projections must match the local Forum category ID set exactly; missing/extra IDs, foreign parent references, cycles and excessive depth return typed validation failures before visibility filtering or local overrides. The shared `category_hierarchy` validator applies to base visibility, richer audience ancestry, category trees, direct Search subtree scope and read-tracking subtree enumeration. Tree metadata additionally rejects duplicate/foreign snapshot identities before sorting; matching vector length alone is not enough.
- Storefront selected-category reads keep the `forum_categories:list` boundary all the way through metadata loading. Ordinary selected-category owner reads still require `forum_categories:read`. The private `CategorySelectedReadAccess` mode preserves that distinction without adding permissions to a security snapshot. The `taxonomy_read_wiring` suite includes authored, unexecuted least-privilege, malformed-provider and post-write transactional-failure regressions; `category_hierarchy` has authored pure validation/depth tests.
- Category Media identities are typed Taxonomy references. Media remains the binary lifecycle owner; Forum does not resolve Category-cover Media capability, store Media URLs/paths, read Media persistence, or expose a duplicate cover error surface.
- Forum attachment relations remain separate and use the Media owner-reference retention contract.

### Mention and quote revision contract
- Mention extraction walks the canonical `RichTextDocument` tree and ignores `codeBlock` nodes, code-marked text, and email-address `@` tokens.
- Forum never parses a format selector, Markdown source, or a second JSON envelope for mentions.
- Ordinary handles use `ProfileService::normalize_handle`; the `moderators` audience is a separate typed target and requires explicit moderation policy.
- Every revision is capped at 32 unique mention targets and 32 unique quote references.
- `resolve_forum_mentions` uses the tenant-scoped `ProfilesReader` contract. Missing, hidden, blocked, private, followers-only, foreign-tenant or mismatched targets all fail with the same safe `FORUM_MENTION_TARGET_UNAVAILABLE` class.
- `ForumQuoteReference` stores target identity plus quoted revision identity; the renderer never infers historical identity from display text.
- `diff_forum_mentions` is deterministic by resolved user identity. Only added targets produce `ForumMentionEventCandidate`; unchanged and removed targets never become delivery candidates.
- Replaying the same source revision with changed targets fails closed. A byte-identical replay produces no added candidates.
- FORUM-12A contains no persistence, event publication, notification call or transport surface.
- Run `node scripts/verify/verify-forum-mention-contract.mjs` after changing this boundary.
### Mention and quote persistence contract
- `forum_relation_revisions` assigns one globally unique immutable identity to each persisted mention/quote projection for a tenant, source target and locale.
- `forum_user_mentions`, `forum_audience_mentions` and `forum_quotes` are append-only child rows keyed by the complete source identity and relation revision.
- Quote rows retain the quoted target and globally unique quoted relation revision; PostgreSQL and SQLite reject tenant, kind or target mismatches.
- The canonical topic/reply command owner appends the projection revision after the source document has been normalized and stored; source INSERT seed triggers first record one `legacy` baseline revision for every body written without a relation projection, so bodies from imports, forks, splits, translation applies and pre-FORUM-12B2 data stay readable and reconcilable.
- A `legacy` baseline carries no mention, audience or quote rows: it states that the body exists and that no projection was derived from it, and mention reconciliation accepts it as a valid fingerprint instead of reporting drift.
- Migrations do not synthesize empty relation identities or backfill placeholder fingerprints.
- The crate-private `MentionRelationService` separates profile-dependent `prepare` from transaction-only `persist_in_tx`; it is an owner implementation seam, not public persistence API.
- `prepare` resolves handles through `ProfilesReader` and computes a SHA-256 fingerprint over the canonical serialized document, resolved targets, and quote identities.
- `persist_in_tx` locks the source stream, re-reads the persisted body in the same transaction, rejects prepared/body mismatch and writes the revision plus all child rows atomically.
- Topic and reply create/edit owner commands prepare before opening the transaction and call `persist_in_tx` immediately after the canonical body write and before counters, semantic events and commit.
- An identical latest fingerprint must also match the persisted target snapshot; only then does replay return the existing relation revision with no added targets.
- `FORUM_QUOTE_TARGET_UNAVAILABLE` safely covers missing, foreign-tenant or mismatched quoted revision identity without exposing target existence.
- Run `node scripts/verify/verify-forum-mention-persistence.mjs` and `node scripts/verify/verify-forum-mention-integration.mjs` after changing this boundary.
### Mention events and relation owner read
- `ForumMentionEvent` is a sealed `rustok-events` family with v1 `forum.mention.user_added` and `forum.mention.audience_added` contracts.
- Event payloads contain the source kind/ID/relation revision/locale plus resolved user ID or typed audience; they contain no contact data, profile handle snapshot or rendered body.
- Only `MentionRelationSyncResult.added_user_ids` and `added_audiences` are published. Identical replay, removed targets and unchanged targets emit nothing.
- Each event is written through `TransactionalEventBus` to the canonical outbox and to `forum_domain_events` with the same event UUID inside the source owner transaction.
- PostgreSQL and SQLite journal constraints accept both event types and keep the journal append-only.
- `ForumRelationReadService` returns latest or exact tenant/source/locale revision snapshots, bounded to 32 mention targets and 32 quotes.
- Relation owner reads expose user IDs, audiences and revision-bound quotes, but not `handle_snapshot` or `projection_fingerprint`.
- Invalid or unavailable relation identity returns `FORUM_RELATION_REVISION_UNAVAILABLE` without disclosing cross-tenant existence.
- No REST or GraphQL relation endpoint is added in FORUM-12C.
- Run `node scripts/verify/verify-forum-mention-events.mjs` after changing this boundary.
### Topic slug rename
- `TopicService::rename_slug` owns localized slug mutation and is exposed as `renameForumTopicSlug` with `RenameForumTopicSlugGraphqlInput` and `GqlForumTopicSlugRename`.
- GraphQL requires the enabled Forum module and `forum_topics:update`; tenant authority comes from routed `TenantContext`, and any optional tenant argument must match it. Ownership remains owner-defined, never bypassed by the adapter.
- GraphQL composes the configured topic owner through `ForumGraphqlRuntimeData`, including its transactional author edit window. The locked owner rechecks ownership, the window and archived state before writing.
- The route owner locks the localized route, writes one immutable old-route alias with reason `Topic slug changed`, and updates the canonical slug in the same transaction as topic projection invalidation.
- An exact normalized replay returns `changed = false`; a changed write returns its alias identity, previous slug/path and canonical descriptor. Deleted and merged route resolution remain owned by the route lifecycle services.
- The adapter never reads route-alias tables or implements route business logic. Schema and owner regression sources exist; Rust execution remains a release check in a toolchain-enabled environment.
- Source checks: `node scripts/verify/verify-forum-topic-slug-rename-owner.mjs` and `node scripts/verify/verify-forum-topic-slug-rename-graphql-transport.mjs`.

### Quote owner commands
- `SetForumQuotesInput` contains an exact source locale and a full replacement list of typed `ForumQuoteReferenceInput` values.
- Exact duplicates are normalized deterministically and the submitted set is capped at 32 references.
- An empty list explicitly clears quotes while retaining mentions extracted from the unchanged canonical body.
- `ForumQuoteCommandService` requires the corresponding topic/reply update owner scope, prepares outside the transaction, persists the immutable relation revision and materializes the bounded response before commit.
- Standalone quote commands recheck source ownership, deletion, parent lifecycle and the author edit window inside the write transaction under category -> topic -> source locking. Archived parents reject all quote edits; locked parents reject reply quote edits. Failed transactional settings reads reject the command without a new relation revision.
- REST and GraphQL obtain the quote owner from their configured runtime, including both static and transactional settings providers, rather than constructing an unconfigured service. Omitting the required standalone quote list is rejected; inline omitted-update semantics remain separate.
- Identical replacement replays the current relation revision; missing, cross-tenant or mismatched quote targets use `FORUM_QUOTE_TARGET_UNAVAILABLE`.
- REST entry points: `PUT /api/forum/topics/{id}/quotes` and `PUT /api/forum/replies/{id}/quotes`.
- GraphQL entry points: `setForumTopicQuotes` and `setForumReplyQuotes`.
- Run `node scripts/verify/verify-forum-quote-commands.mjs` after changing this boundary.
### Inline quote create and edit commands
- Separate `Create*CommandInput` and `Update*CommandInput` DTOs add typed quote references without changing existing Rust create/update structs.
- Create commands treat omitted quotes as an empty initial set.
- Update commands preserve the latest exact-locale quote set when `quotes` is omitted, explicitly clear with `quotes: []`, and fully replace with a non-empty list.
- Existing `TopicService::create/update` and `ReplyService::create/update` convert legacy DTOs to command DTOs, so legacy body edits preserve quotes.
- Omitted-update preservation records the expected relation revision, locks the active source inside the transaction and returns retryable `FORUM_RELATION_REVISION_CONFLICT` if the stream changed concurrently.
- REST uses the existing topic/reply create and update routes with command DTOs.
- GraphQL exposes `createForumTopicWithQuotes`, `updateForumTopicWithQuotes`, `createForumReplyWithQuotes` and `updateForumReplyWithQuotes` alongside the standard content mutations.
- Soft-deleted sources reject inline relation updates; quote references remain bounded to 32 raw entries.
- Run `node scripts/verify/verify-forum-quote-commands.mjs` after changing this boundary.
### CreateTopicInput
- Added: `slug: Option<String>`
### ListRepliesFilter (new)
- Reply pagination: `page`, `per_page`, `locale`
### ModerationService
- Signatures `approve_reply`, `reject_reply`, `hide_reply`, `pin_topic`, `unpin_topic` now accept `tenant_id: Uuid`
- `close_topic`, `archive_topic` now accept `tenant_id: Uuid`
- Added `mark_solution(tenant_id, topic_id, reply_id, security)` and `clear_solution(tenant_id, topic_id, security)`
- `FORUM-20AY` adds `with_audience_facts` plus context-aware variants for every topic/reply moderation command and moderator solution fallback.
### Moderation GraphQL mutations
- Added: `pinForumTopic(id: UUID!, pinned: Boolean!)`, `lockForumTopic(id: UUID!, locked: Boolean!)`, `closeForumTopic(id: UUID!)`, `reopenForumTopic(id: UUID!)` with `Permission::FORUM_TOPICS_MANAGE`.
- Added: `deleteForumReply(id: UUID!)` with `Permission::FORUM_REPLIES_DELETE` and `restoreForumReply(id: UUID!)` with `Permission::FORUM_REPLIES_MANAGE`.
- `ReplyService::restore` and both REST/GraphQL adapters reject an archived parent with `FORUM_TOPIC_ARCHIVED` (REST HTTP 409), for ordinary and previously accepted replies alike. Reopen the parent explicitly before retrying; the owner never implicitly reopens it or drops historical solution metadata. Rejected restoration preserves the tombstone, delete snapshot, revisions, aggregates and outbox. Successful restoration serializes category → topic → solution scope → reply and retains the existing atomic snapshot replay. See [parent admission decision](../../../DECISIONS/2026-11-26-forum-reply-restore-parent-admission.md).
- Added: `approveForumReply(replyId: UUID!, topicId: UUID!)`, `rejectForumReply(replyId: UUID!, topicId: UUID!)` with `Permission::FORUM_REPLIES_MODERATE`.
### Trusted moderation adapter effects
- Approved reply hide/reject uses the shared publication effects: persist canonical status, clear an accepted-solution relation, then update topic/category/member projections. Reply and solution member counts change together; archived parents do not release category/member contributions twice.
- Admission follows category → parent topic → solution → subject/revision scope order, rechecks a discovered reply parent and retains reviewed-revision fencing. SQLite claims its writer before parent reads; PostgreSQL keeps Serializable isolation.
- Hide/reject and public/solution removal publish topic projection invalidation atomically with existing category/status events. Exact receipt replay and same-target hide/reject remain no-write paths. No transport payload or Leptos/Next.js contract changes.
- Run `npm run verify:forum:moderation-effects` and `npm run test:verify:forum:moderation-effects` for supplemental guards. `cargo test -p rustok-forum --test logic_review_remediation_sqlite trusted_moderation_effects` mounts the shared SQLite/PostgreSQL owner matrix; set `RUSTOK_FORUM_TEST_DATABASE_URL` for PostgreSQL. Rust service/concurrency execution remains required. See [effects verification](./docs/forum-moderation-effect-postgres-contract.md).
### Delete policy admission
- Public topic/reply `delete` commands repeat ownership and author-policy admission after their source write fence using transactional Forum settings. Preflight is only an early refusal. Snapshot capture and destructive writes occur after this admission; rejection preserves source/body/revision/solution/statistics state, snapshots and outbox.
- Default-only services (no settings readers) retain canonical defaults. When a host configures a static settings reader, an absent transactional reader fails with `FORUM_TRANSACTIONAL_SETTINGS_UNAVAILABLE` instead of replacing stored policy with defaults; read-only consumers remain supported. A transaction-only configuration fails ordinary and transactional policy reads with `FORUM_STATIC_SETTINGS_UNAVAILABLE`. REST/GraphQL/native composition uses `ForumSettingsProviders::from_optional_readers` to retain both options; only an actually empty pair denotes default-only services. A functioning paired reader with no saved row retains defaults, and reader failures propagate. This shared rule covers every transactional settings consumer, including edit, quote, slug, posting, vote and delete commands; see [Transactional settings capability](../../../DECISIONS/2026-11-26-forum-transactional-settings-capability.md).
- GraphQL default-setting reads (topic order and topic/reply page sizes) propagate the typed `ForumError` unchanged, including aliased fields. Missing capability retains its exact stable code and `retryable: false`; transient reader failure retains `FORUM_CAPABILITY_FAILURE`, `retryable: true` and `retryAfter: 5`, without exposing owner diagnostics. `cargo test -p rustok-forum --test graphql_settings_error_contract` exercises all eight resolver branches; runtime execution remains required.
- Staff/system All-scoped callers keep their false-policy exemption and moderation/system snapshot reasons; capability failures are not silently ignored.
- Reply direct deletion and internal moderation removal share one locking path and effects core. The internal API remains authorized by its caller and does not inherit author self-deletion policy. Both REST and GraphQL already supply configured owner services; no client payload changes or migration is required.
- Run `npm run verify:forum:delete-policy` and `npm run test:verify:forum:delete-policy` for supplemental source checks. The shared owner matrix is `cargo test -p rustok-forum --test logic_review_remediation_sqlite direct_delete_rechecks_policy`; set `RUSTOK_FORUM_TEST_DATABASE_URL` for PostgreSQL. See [delete policy decision](../../../DECISIONS/2026-11-26-forum-delete-policy-admission.md).
### Topic delete-snapshot restoration
- `TopicService::restore` restores the exact pre-delete status, lock and thread snapshot, including an already-archived topic's original accepted solution. It does not implicitly reopen the topic or revive replies deleted independently before the thread deletion.
- Migration 47 permits an archived solution INSERT only while matching tenant/topic/reply delete snapshots admit that exact previously approved target. Parent and reply must already be live; ordinary archived INSERTs and target UPDATEs remain rejected. Successful restore consumes the snapshots in the same transaction, closing this replay admission.
- Archived restoration retains zero category/member contributions and emits no new publication event. An explicit later reopen restores those contributions once. Marker identity and time are replayed from the snapshot by the owner; no new API fields or client-specific restore paths are introduced. See [topic solution snapshot replay decision](../../../DECISIONS/2026-11-26-forum-topic-restore-solution-replay.md).
### VoteService
- Added `set_topic_vote(tenant_id, topic_id, security, value)` and `clear_topic_vote(tenant_id, topic_id, security)`
- Added `set_reply_vote(tenant_id, reply_id, security, value)` and `clear_reply_vote(tenant_id, reply_id, security)`
### SubscriptionService
- Added `set_category_subscription(tenant_id, category_id, security)` and `clear_category_subscription(tenant_id, category_id, security)`
- Added `set_topic_subscription(tenant_id, topic_id, security)` and `clear_topic_subscription(tenant_id, topic_id, security)`
### UserStatsService
- Added `get(tenant_id, security, user_id)` for tenant-scoped forum statistics read-path
- Internal write-path helpers synchronize `topic_count`, `reply_count`, `solution_count`

## Locale fallback chain
Translation lookup order: `requested → explicit fallback → "en" → first available`.
The `effective_locale` field indicates which locale was actually returned.
Quote owner commands and inline quote preservation intentionally use an exact existing source locale and do not use fallback.

## Slug contract
- `CategoryResponse` / `CategoryListItem` return locale-aware slug at the `forum_category_translation` level; the slug follows the same resolved translation as `name` / `description`.
- `TopicResponse` / `TopicListItem` return a stable topic slug. When creating a new topic translation, the slug is copied from the seed translation; subsequent localized slug changes use `TopicService::rename_slug`.
- ID-based content APIs remain available. Public topic-route resolution uses the stable short ID plus localized slug through `ForumTopicRouteService`, retaining shared locale fallback and audience admission; slug text alone is not topic identity.

## Events
Publishes forum domain events through the outbox pipeline:
- `ForumTopicCreated` — when a topic is created
- `ForumTopicReplied` — when a reply is added
- `ForumTopicStatusChanged` — when topic status changes (close/archive)
- `ForumTopicPinned` — when topic is pinned/unpinned
- `ForumReplyStatusChanged` — when a reply is moderated (approve/reject/hide)
- `forum.mention.user_added` — once for each newly added resolved user target
- `forum.mention.audience_added` — once for each newly added typed audience target

Legacy Forum lifecycle events remain root `DomainEvent` variants. Mention events use the sealed `ForumMentionEvent` typed family in `rustok-events`. Forum publishes to the transactional outbox and never invokes Notifications synchronously.

## Owner Service Boundary
- Public topic and reply workflows use the root `TopicService` and `ReplyService` facade exports.
- Raw `services::topic`, `services::reply`, `topic_owner`, `reply_owner` and `mention_relation` implementations are crate-private.
- Public facades expose explicit create/read/update/list methods and never implement `Deref` to an implementation service.
- Topic/reply deletion must use facade `delete` methods so tombstones, counters and semantic events remain atomic.
- Active mention/quote persistence and added-target event publication are composed by the same owner write transaction; transports must never invoke `MentionRelationService` or event publishing directly.
- Quote replacement is exposed only through `ForumQuoteCommandService`; inline quote create/edit is exposed through topic/reply command facades.
- The legacy facade methods convert into command DTOs and preserve existing quotes on body updates.
- Relation snapshots are read only through `ForumRelationReadService` or materialized inside an active owner transaction.
- Moderation transports must call `ModerationService`; audience evaluation remains owner-side, inside the locked writer transaction and before domain effects.
- Run `node scripts/verify/verify-forum-owner-boundary.mjs` after changing topic/reply service visibility or workspace consumers.

## Dependencies on Other RusToK Crates
- `rustok-content`
- `rustok-core`
- `rustok-taxonomy` for canonical Category identity/copy/hierarchy/routes/presentation and `rustok-media` for Forum attachment owner-reference retention
- `rustok-events` for the sealed Forum mention event family
- `rustok-outbox`
- `rustok-profiles` for tenant-scoped mention handle resolution through `ProfilesReader`

## Common AI Mistakes
- Incorrectly uses moderation limits/constants from `constants`.
- Confuses the category/topic/reply hierarchy in entity imports.
- Ignores tenant-boundary in service filters.
- Confuses `locale` (requested) and `effective_locale` (actually used).
- Uses the flat category list to reconstruct hierarchy instead of `CategoryService::tree`.
- Writes `parent_id` or sibling positions directly instead of using category owner commands.
- Writes lifecycle rows parent-first or restores a child beneath an archived ancestor.
- Creates a topic without honoring the category-owned lifecycle and `allows_topics` policy.
- Reimplements Taxonomy Category presentation normalization or persistence inside Forum.
- Reads Taxonomy entity tables directly instead of using the Taxonomy owner projection/mutation ports.
- Treats Category-cover lifecycle as Forum-owned instead of consuming Media/Taxonomy owner contracts.
- Parses mentions from code blocks or code-marked structural text.
- Resolves mention handles by querying profile tables or by trusting display labels instead of `ProfilesReader`.
- Emits mention delivery for unchanged targets or rewrites quote history to the latest revision.
- Calls Notifications from the Forum transaction instead of publishing a typed owner event.
- Uses separate identities for the outbox and Forum journal copies of one mention event.
- Exposes `handle_snapshot`, projection fingerprint or source body through the relation owner read.
- Updates a persisted mention/quote row instead of appending a new relation revision.
- Writes a source document without composing relation persistence through the same command owner.
- Persists a prepared relation projection without revalidating the source body inside the owner transaction.
- Lets quote transports import `MentionRelationService`, `PreparedMentionRelations` or `persist_in_tx`.
- Treats omitted update quotes as an empty list and silently clears relations.
- Preserves an out-of-date quote snapshot without expected-revision CAS.
- Returns a quote command response through a post-commit read that can fail after the write committed.
- Imports raw topic/reply implementation modules instead of the root owner facades.
- Treats `forum_user_stats` activity counters as Forum trust state.
- Builds reply-create audience identity from DTO fields or bypasses the exact transport context and host-composed `ReplyService`.
- Reuses `forum_topic_audience_*` visibility rows for topic-local reply-create narrowing instead of the separate owner policy.
- Applies moderator audience policy to a topic author's owned solution command instead of only the moderation fallback.
- Treats a pre-transaction moderation preview as write authorization, evaluates audience outside the locked writer transaction, or queries Channel/Groups tables directly.
- Passes methods to `ModerationService` without `tenant_id` — it is now required.

## Minimum Contract Set

### Input DTOs/Commands
- `CreateTopicInput`, `UpdateTopicInput`, `CreateReplyInput`, and `UpdateReplyInput` accept one `RichTextDocument` field.
- Separate command DTOs carry inline quotes and are consumed by transport adapters and facade conversions.
- There are no format selectors, alternate content fields, or compatibility DTO variants.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.
- Category tree reads fail closed for oversized, excessive-depth, untranslated, cyclic, disconnected, foreign-parent or invalid archive hierarchies.
- Category move/reorder commands use a per-tenant transaction order and never persist a partial sibling normalization.
- Category write paths enforce depth 16 at the database boundary; metadata updates cannot change sibling placement.
- Category subtree lifecycle is tenant-scoped, atomic, idempotent and enforced at the database boundary for category hierarchy and topic placement.
- Category topic policy is tenant-scoped and enforced at the database boundary for topic inserts and category reassignment.
- Category topic-create and reply-create audience layers are normalized, independently inherited, bounded, immutable on direct update and separate from content visibility.
- Topic-local reply-create audience is a separate normalized final narrowing layer evaluated only after every inherited category reply-create layer.
- Category moderation audience is normalized, inherited as a conjunction, and evaluated before all moderator-owned topic/reply/solution writes while preserving the topic-author solution owner path.
- Topic/reply create transports derive exact tenant, actor, claims, locale, deadline, and route channel only from authenticated runtime contexts and use the same host-published optional audience facts port.
- Category presentation values are normalized and owned by Taxonomy; Forum consumes the owner projection and never creates a second persistence validator.
- Mention extraction is bounded, format-aware and code/escape-safe; profile resolution is tenant-scoped and privacy fail-closed.
- Mention revision diffs are immutable on replay and only added resolved targets become owner events.
- Relation revisions and mention/quote children are append-only, tenant-bound and atomically matched to the persisted source body.
- Source INSERT seeding preserves one relation identity for every persisted topic/reply locale during the B1-to-B2 rollout window.
- Mention events and their relation revision are committed atomically with one identity shared between outbox and Forum journal.
- Bounded relation reads are tenant/source/locale scoped and never expose handle snapshots or replay fingerprints.
- Quote replacement is exact-locale, owner-scoped, bounded to 32 references and materializes its response before commit.
- Inline body edits preserve omitted quotes by expected relation revision and conflict rather than overwrite a concurrent replacement.
- Quote relations retain target revision identity, reject source/target mismatches and cannot self-reference their own source revision.
- Attachment relations use a dedicated per-target/locale CAS head; content source revisions remain provenance only.
- Initial attachment persistence creates relation revision 1, including an explicit empty-set commit; later replacements require the exact current token.
- Attachment order is bounded to 32 contiguous zero-based positions and is unique per target/locale.
- Forum relation rows store only Media identity; durable Media owner references are retained before Forum commit and released after Forum commit.
- Stable Media reference IDs include tenant, target, locale, position and media identity; usage/caption changes do not retarget the owner reference.
- Ambiguous Forum commits and post-commit Media release failures preserve conservative holds rather than attempting unsafe compensation.
- Public topic/reply access is restricted to explicit owner facades; persistence modules and owner implementations are not part of the external contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.
- `forum.mention.user_added` and `forum.mention.audience_added` are emitted only for targets added by a newly persisted relation revision.
- Quote-only replacement and preserved body edits emit nothing for unchanged mentions and never call Notifications synchronously.
- Forum records the same event UUID in the canonical outbox and append-only owner journal.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- GraphQL selected category/topic reads, topic-route resolution/disclosure, category-route resolution/audience context/visibility and member-card validation/composition propagate `ForumError` through `?` or `error.into()`, not `Error::new(error.to_string())` or `internal_error(error.to_string())`. `ForumGraphqlErrorExtension` retains the source's code and retryability even when the field is aliased. Storage failures stay redacted, retryable `FORUM_INTERNAL_ERROR`; validation stays nonretryable `FORUM_VALIDATION_FAILED`. Existing privacy-safe missing/denied/not-found branches still return `null`, and empty member-card input still returns an empty list. Foreign owner errors (such as raw Profiles/Channel errors) keep their separate mappings; this rule does not relabel them as Forum failures. Run `npm run verify:forum:graphql-error-provenance` and its `test:verify` counterpart; `graphql_settings_error_contract` contains the authored, not-yet-executed resolver regressions.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
- Optional capability absence uses `ForumError::CapabilityUnavailable` and a stable owner-specific code; actual provider failures use `ForumError::CapabilityFailure` and preserve source code and retryability.
- Missing or unauthorized mention targets share `FORUM_MENTION_TARGET_UNAVAILABLE` so the contract does not expose a profile-existence oracle.
- Missing or mismatched quoted relation revisions share `FORUM_QUOTE_TARGET_UNAVAILABLE` so quote validation does not expose a cross-tenant existence oracle.
- Invalid, absent or foreign relation revision identities share `FORUM_RELATION_REVISION_UNAVAILABLE`.
- Attachment relation CAS conflicts return retryable `FORUM_RELATION_REVISION_CONFLICT`; stale content provenance returns retryable `FORUM_ATTACHMENT_SOURCE_REVISION_CONFLICT`; persistence invariant violations return `FORUM_ATTACHMENT_RELATION_INVARIANT` and revision exhaustion returns `FORUM_ATTACHMENT_RELATION_REVISION_EXHAUSTED`.
- Media retention/release provider failures remain `ForumError::CapabilityFailure` with the Media source code and retryability.
- A stale omitted-update quote snapshot returns retryable `FORUM_RELATION_REVISION_CONFLICT`; REST maps it to HTTP 409.
- Forum chooses its engagement mechanism through the tenant-scoped `forum.useReactions` setting. `false` keeps the internal `VoteService` active; `true` selects the shared Reactions module. The Reactions module may remain enabled for other modules without changing Forum behavior.
