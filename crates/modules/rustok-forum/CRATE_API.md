# rustok-forum / CRATE_API

## Public Modules
`category_presentation`, `constants`, `controllers`, `dto`, `entities`, `error`, `graphql`, `locale`, `mentions`, `services`.

## Primary Public Types and Signatures
- `pub struct ForumModule`
- `pub struct CategoryService`, `TopicService`, `ReplyService`, `ModerationService`, `SubscriptionService`, `UserStatsService`, `VoteService`
- `pub struct ForumRelationReadService`
- `ForumRelationReadService::get(tenant_id, security, ForumRelationSnapshotQuery) -> ForumRelationSnapshotResponse`
- `pub struct ForumQuoteCommandService`
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
- `CategoryCoverMediaCandidate`, `normalize_category_icon_key`, `validate_category_cover_candidate`
- `resolve_category_cover_for_write(media_port, context, media_id, alt) -> ForumResult<MediaImageDescriptor>`
- `hydrate_category_cover_for_read(media_port, context, media_id, alt) -> ForumResult<Option<MediaImageDescriptor>>`
- `extract_forum_mention_candidates(document: &RichTextDocument, policy) -> ForumResult<ForumMentionCandidates>`
- `resolve_forum_mentions(profiles, tenant_id, candidates, requested_locale, tenant_default_locale) -> ForumResult<ForumResolvedMentions>`
- `validate_forum_quote_references(source, references) -> ForumResult<Vec<ForumQuoteReference>>`
- `diff_forum_mentions(previous, current) -> ForumResult<ForumMentionDiff>`
- `ForumRevisionIdentity`, `ForumMentionRevisionProjection`, `ForumMentionEventCandidate`, `ForumQuoteReference`
- `ForumQuoteTargetKindInput`, `ForumQuoteReferenceInput`, `SetForumQuotesInput`
- `CreateTopicCommandInput`, `UpdateTopicCommandInput`, `CreateReplyCommandInput`, `UpdateReplyCommandInput`
- `ForumRelationSnapshotQuery`, `ForumRelationSnapshotResponse`, `ForumRelationQuoteResponse`
- `pub mod graphql` -> `ForumQuery`, `ForumMutation`
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
### CategoryResponse
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `is_subscribed: bool`
### CategoryListItem
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `is_subscribed: bool`
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
- Run `node scripts/verify/verify-forum-category-reply-create-audience-policy.mjs`, `node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs` after changing this boundary.
### Topic reply-create audience narrowing
- `ForumTopicReplyCreateAudiencePolicyService` stores an optional normalized topic-local reply-create layer independently from `forum_topic_audience_*` content visibility.
- Managed `get` and atomic replacement `set` require `forum_topics:manage`; empty constraints clear only the local topic layer.
- `FORUM-20AX` evaluates all inherited category layers followed by the optional topic layer, so a topic can narrow but never broaden its category reply-create policy.
- Internal authorization identifies the exact category or topic layer that denied the actor while retaining one generic public denial.
- Existing legacy and inline-quote owner paths, plus their composed GraphQL and REST transports, inherit the topic check without DTO or transport changes.
- PostgreSQL and SQLite enforce tenant/topic ownership, typed relations, immutable rows, and bounded direct channel/group/allow/deny inserts.
- Run `node scripts/verify/verify-forum-topic-reply-create-audience-policy.mjs`, `node scripts/verify/verify-forum-reply-create-audience-enforcement.mjs`, and `node scripts/verify/verify-forum-reply-create-audience-transport-composition.mjs` after changing this boundary.
### Category moderation audience policy
- `ForumCategoryModerationAudiencePolicyService` stores a normalized category-local moderator audience independently from content visibility and posting eligibility.
- `FORUM-20AY` evaluates every inherited root-to-category layer before topic/reply moderation writes; explicit deny retains precedence and a child cannot broaden an ancestor.
- Managed `get` and atomic replacement `set` require `forum_categories:manage`; empty constraints restore inheritance.
- Existing context-free moderation methods preserve unrestricted, role-only, and explicit-user behavior. Trust, Channel, or Groups selectors require exact context plus `SharedForumAudienceFactsPort` and otherwise fail closed.
- `ModerationService::with_audience_facts` and context-aware command variants cover reply status, topic pin/lock/status, and solution commands before their write transactions.
- Topic authors retain the existing owned-update path for marking or clearing solutions; only the moderator fallback is audience-gated.
- PostgreSQL and SQLite enforce tenant/category ownership, immutable rows, bounded relations, and one shared advisory key for owner replacement and direct bounded inserts.
- GraphQL/REST exact context composition remains `FORUM-20AZ`; no moderation DTO changed in `FORUM-20AY`.
- Run `node scripts/verify/verify-forum-moderation-audience-policy.mjs` after changing this boundary.
### Category presentation contract
- Existing `icon` storage is interpreted as an `icon_key` and accepts only a bounded lowercase kebab-case semantic token at the database write boundary.
- Category colors remain bounded hexadecimal colors; CSS declarations and arbitrary color expressions are rejected.
- `CategoryCoverMediaCandidate` is a transport-neutral Media-to-Forum validation input and carries only media identity, tenant, MIME, size, dimensions and `MediaImageDescriptor`.
- `validate_category_cover_candidate` rejects foreign tenants, unsupported image MIME, oversized or dimensionless images, descriptor mismatch and non-direct-public delivery.
- `resolve_category_cover_for_write` calls the Media owner port and fails with stable code `FORUM_CATEGORY_COVER_MEDIA_CAPABILITY_UNAVAILABLE` when Media is not composed; it never treats a missing capability as a clear-cover command.
- `hydrate_category_cover_for_read` returns `None` only for the explicit Media-disabled profile. Media not-found, timeout, storage and other provider failures remain typed `ForumError::CapabilityFailure` values with source code and retryability.
- Forum does not accept or store cover URLs, storage paths, drivers, credentials or blobs.
- Persistent `cover_media_id` writes remain disabled until the Media owner contract publishes quarantine/deletion state.
- Run `node scripts/verify/verify-forum-category-presentation.mjs` after changing this boundary.
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
- Relation revisions are created only by the canonical topic/reply command owner after the source document has been normalized and stored.
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
### Quote owner commands
- `SetForumQuotesInput` contains an exact source locale and a full replacement list of typed `ForumQuoteReferenceInput` values.
- Exact duplicates are normalized deterministically and the submitted set is capped at 32 references.
- An empty list explicitly clears quotes while retaining mentions extracted from the unchanged canonical body.
- `ForumQuoteCommandService` requires the corresponding topic/reply update owner scope, prepares outside the transaction, persists the immutable relation revision and materializes the bounded response before commit.
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
- `TopicResponse` / `TopicListItem` return a stable topic slug. When creating a new topic translation, the slug is copied from the seed-translation, unless a separate topic-level slug workflow is explicitly introduced.
- The current forum public contract remains ID-based: the forum API does not promise lookup by slug. If such a read-path is added later, it must use the same locale fallback contract as the rest of the forum read-path.

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
- Moderation transports must call `ModerationService`; audience evaluation remains owner-side and precedes every moderation write transaction.
- Run `node scripts/verify/verify-forum-owner-boundary.mjs` after changing topic/reply service visibility or workspace consumers.

## Dependencies on Other RusToK Crates
- `rustok-content`
- `rustok-core`
- `rustok-media` for transport-neutral image descriptors, owner read-port resolution and optional-capability degradation
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
- Treats category `icon` as a CSS class, URL or markup instead of a semantic icon key.
- Stores a category image URL/path or reads Media tables instead of using the Media owner port.
- Swallows a Media port failure as an absent category cover instead of degrading only when Media is not composed.
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
- Opens a moderation write transaction before category audience authorization or queries Channel/Groups tables directly.
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
- Category icon/color values are bounded safe tokens; cover media candidates are tenant-scoped and transport-neutral.
- Category cover writes fail closed when Media is unavailable; reads degrade only for an explicitly absent optional Media owner and never swallow provider errors.
- Mention extraction is bounded, format-aware and code/escape-safe; profile resolution is tenant-scoped and privacy fail-closed.
- Mention revision diffs are immutable on replay and only added resolved targets become owner events.
- Relation revisions and mention/quote children are append-only, tenant-bound and atomically matched to the persisted source body.
- Source INSERT seeding preserves one relation identity for every persisted topic/reply locale during the B1-to-B2 rollout window.
- Mention events and their relation revision are committed atomically with one identity shared between outbox and Forum journal.
- Bounded relation reads are tenant/source/locale scoped and never expose handle snapshots or replay fingerprints.
- Quote replacement is exact-locale, owner-scoped, bounded to 32 references and materializes its response before commit.
- Inline body edits preserve omitted quotes by expected relation revision and conflict rather than overwrite a concurrent replacement.
- Quote relations retain target revision identity, reject source/target mismatches and cannot self-reference their own source revision.
- Public topic/reply access is restricted to explicit owner facades; persistence modules and owner implementations are not part of the external contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.
- `forum.mention.user_added` and `forum.mention.audience_added` are emitted only for targets added by a newly persisted relation revision.
- Quote-only replacement and preserved body edits emit nothing for unchanged mentions and never call Notifications synchronously.
- Forum records the same event UUID in the canonical outbox and append-only owner journal.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
- Optional capability absence uses `ForumError::CapabilityUnavailable` and a stable owner-specific code; actual provider failures use `ForumError::CapabilityFailure` and preserve source code and retryability.
- Missing or unauthorized mention targets share `FORUM_MENTION_TARGET_UNAVAILABLE` so the contract does not expose a profile-existence oracle.
- Missing or mismatched quoted relation revisions share `FORUM_QUOTE_TARGET_UNAVAILABLE` so quote validation does not expose a cross-tenant existence oracle.
- Invalid, absent or foreign relation revision identities share `FORUM_RELATION_REVISION_UNAVAILABLE`.
- A stale omitted-update quote snapshot returns retryable `FORUM_RELATION_REVISION_CONFLICT`; REST maps it to HTTP 409.
