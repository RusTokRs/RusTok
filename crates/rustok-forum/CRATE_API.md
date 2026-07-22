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
- `CategoryService::tree(tenant_id, security, CategoryTreeQuery) -> CategoryTreeResponse`
- `CategoryService::move_category(tenant_id, category_id, security, MoveCategoryInput) -> MoveCategoryResponse`
- `CategoryService::reorder_siblings(tenant_id, security, ReorderCategorySiblingsInput) -> ReorderCategorySiblingsResponse`
- `CategoryService::archive_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::restore_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::topic_policy(tenant_id, category_id, security) -> CategoryTopicPolicyResponse`
- `CategoryService::set_topic_policy(tenant_id, category_id, security, UpdateCategoryTopicPolicyInput) -> CategoryTopicPolicyResponse`
- `CategoryCoverMediaCandidate`, `normalize_category_icon_key`, `validate_category_cover_candidate`
- `resolve_category_cover_for_write(media_port, context, media_id, alt) -> ForumResult<MediaImageDescriptor>`
- `hydrate_category_cover_for_read(media_port, context, media_id, alt) -> ForumResult<Option<MediaImageDescriptor>>`
- `extract_forum_mention_candidates(body, body_format, locale, policy) -> ForumResult<ForumMentionCandidates>`
- `resolve_forum_mentions(profiles, tenant_id, candidates, requested_locale, tenant_default_locale) -> ForumResult<ForumResolvedMentions>`
- `validate_forum_quote_references(source, references) -> ForumResult<Vec<ForumQuoteReference>>`
- `diff_forum_mentions(previous, current) -> ForumResult<ForumMentionDiff>`
- `ForumRevisionIdentity`, `ForumMentionRevisionProjection`, `ForumMentionEventCandidate`, `ForumQuoteReference`
- `ForumQuoteTargetKindInput`, `ForumQuoteReferenceInput`, `SetForumQuotesInput`
- `ForumRelationSnapshotQuery`, `ForumRelationSnapshotResponse`, `ForumRelationQuoteResponse`
- `pub mod graphql` -> `ForumQuery`, `ForumMutation`
- `pub mod controllers` -> `axum_router()`
- Public DTOs/constants from `dto::*` and `constants::*`
- `pub enum ForumError`, `pub type ForumResult<T>`
- `pub mod locale` — helpers `resolve_translation`, `resolve_body`, `available_locales`

## DTO changes (current)
### TopicResponse
- Added: `requested_locale`, `effective_locale`, `available_locales`, `slug`, `author_id`, vote fields, subscription state and `solution_reply_id`.
### TopicListItem
- Added: requested/effective locale, available locales, stable slug, author, vote, subscription and solution fields.
### ReplyResponse / ReplyListItem
- Added: effective locale, author, parent relation, vote fields and solution state.
### CategoryResponse / CategoryListItem
- Added: requested/effective locale, available locales and subscription state.

### Category tree
- `CategoryTreeQuery`, `CategoryBreadcrumb`, `CategoryTreeNode`, `CategoryTreeResponse` expose the complete tenant hierarchy in deterministic `(position, id)` sibling order.
- One owner call is bounded to 512 nodes and zero-based depth 16.
- Nodes include parent/depth/child metadata, localized breadcrumbs, topic policy, archive state and nested children.
- REST: `GET /api/forum/categories/tree`.
- GraphQL: `forumCategoryTree(tenantId, locale, fallbackLocale)`.
- Oversized, excessive-depth, untranslated, cyclic, disconnected or foreign-parent hierarchies fail closed.

### Category placement commands
- `MoveCategoryInput` and `ReorderCategorySiblingsInput` route all placement through tenant-serialized owner commands.
- Moves reject cycles, foreign parents and depth overflow; reorder requires the complete sibling set exactly once.
- REST: `PUT /api/forum/categories/{id}/move`, `PUT /api/forum/categories/reorder`.
- Generic category update rejects `position`.

### Category subtree lifecycle and topic policy
- `archive_subtree` writes descendants before ancestors; `restore_subtree` removes ancestor lifecycle rows before descendants.
- REST: `POST /api/forum/categories/{id}/archive-subtree`, `POST /api/forum/categories/{id}/restore-subtree`.
- GraphQL: `archiveForumCategorySubtree`, `restoreForumCategorySubtree`.
- Topic policy defaults to `allows_topics = true`; PostgreSQL and SQLite reject new topic placement where policy or archive state forbids it.
- REST: `GET/PUT /api/forum/categories/{id}/topic-policy`.
- GraphQL: `forumCategoryTopicPolicy`, `setForumCategoryTopicPolicy`.

### Category presentation contract
- Category `icon` is a bounded lowercase kebab-case semantic key and color is a bounded hexadecimal value.
- `CategoryCoverMediaCandidate` is transport-neutral and carries media identity, tenant, MIME, size, dimensions and `MediaImageDescriptor` only.
- Cover writes fail with `FORUM_CATEGORY_COVER_MEDIA_CAPABILITY_UNAVAILABLE` when Media is not composed.
- Reads degrade only for an explicitly absent optional Media owner; provider failures stay typed and retryability-aware.
- Forum never stores cover URLs, storage paths, credentials or blobs.
- Run `node scripts/verify/verify-forum-category-presentation.mjs` after changing this boundary.

### Mention and quote revision contract
- Markdown extraction ignores fenced code, inline code, escaped text and email-address `@` tokens.
- `rt_json_v1` extraction uses the canonical sanitizer and ignores code nodes/marks.
- Ordinary handles use the Profiles-owned grammar; `moderators` is a typed permission-gated audience.
- Every relation revision is capped at 32 unique mention targets and 32 unique quote references.
- Missing, hidden, blocked, private, followers-only, foreign-tenant or mismatched mention targets share `FORUM_MENTION_TARGET_UNAVAILABLE`.
- Quote references retain target identity and quoted relation revision identity.
- Only added mention targets become event candidates; replay with changed targets fails closed and identical replay emits nothing.

### Mention and quote persistence contract
- `forum_relation_revisions`, `forum_user_mentions`, `forum_audience_mentions` and `forum_quotes` are append-only and tenant/source/locale/revision bound.
- Existing source locales receive a `legacy` relation revision without parsing historical content or reading Profiles tables.
- `MentionRelationService::prepare` resolves profiles outside the owner transaction and computes a replay fingerprint.
- `persist_in_tx` locks and re-reads the source body, validates quotes and atomically appends the revision and children.
- Topic/reply create/edit owner commands persist the projection immediately after the canonical body write and before counters/events/commit.
- Missing or mismatched quote revisions share `FORUM_QUOTE_TARGET_UNAVAILABLE`.
- Run `node scripts/verify/verify-forum-mention-persistence.mjs` and `node scripts/verify/verify-forum-mention-integration.mjs` after changing this boundary.

### Mention events and relation owner read
- `ForumMentionEvent` is a sealed `rustok-events` family with v1 `forum.mention.user_added` and `forum.mention.audience_added`.
- Payloads contain source identity/revision/locale and resolved user or typed audience identity only.
- The exact persisted added-target diff is published; replay, removed and unchanged targets emit nothing.
- One event UUID is stored in the transactional outbox and append-only Forum journal inside the owner transaction.
- `ForumRelationReadService` returns latest or exact tenant/source/locale snapshots bounded to 32 mention targets and 32 quotes.
- Reads expose user IDs, audiences and revision-bound quotes, never handle snapshots or replay fingerprints.
- Invalid relation identity uses `FORUM_RELATION_REVISION_UNAVAILABLE`.

### Quote owner commands
- `SetForumQuotesInput` contains an exact source locale and a full replacement list of typed `ForumQuoteReferenceInput` values.
- `ForumQuoteCommandService` supports existing topic translations and reply bodies; the caller needs the corresponding update owner scope.
- Exact duplicates are normalized deterministically and the unique set is capped at 32.
- An empty list explicitly clears quote relations while retaining mentions extracted from the unchanged canonical body.
- Preparation occurs before opening the transaction; persistence and bounded response materialization occur before commit.
- Identical replacement replays the current immutable relation revision.
- REST: `PUT /api/forum/topics/{id}/quotes`, `PUT /api/forum/replies/{id}/quotes`.
- GraphQL: `setForumTopicQuotes`, `setForumReplyQuotes`.
- Inline quote input on topic/reply create or body-edit remains a later compatible command slice.
- Run `node scripts/verify/verify-forum-quote-commands.mjs` after changing this boundary.

## Locale fallback chain
Translation lookup order is `requested → explicit fallback → en → first available`. The effective locale is returned explicitly. Quote owner commands intentionally require an exact existing locale and do not use fallback.

## Slug contract
- Category slugs follow the resolved localized translation.
- Topic slugs remain stable when a new locale copies its seed translation.
- Public Forum lookup remains ID-based until a localized route owner contract is introduced.

## Events
Forum publishes lifecycle root events plus sealed mention events through the transactional outbox. Forum never invokes Notifications synchronously.

## Owner Service Boundary
- Public topic/reply workflows use root `TopicService` and `ReplyService` facades.
- Raw `services::topic`, `services::reply`, `topic_owner`, `reply_owner` and `mention_relation` remain crate-private.
- Topic/reply deletion uses facade owner methods so tombstones, counters and events stay atomic.
- Mention/quote persistence and added-target event publication are composed inside owner transactions.
- Quote replacement is exposed only through `ForumQuoteCommandService`; REST and GraphQL never import `MentionRelationService` or `persist_in_tx`.
- Relation snapshots are read through `ForumRelationReadService` or materialized in the active quote owner transaction.
- Run `node scripts/verify/verify-forum-owner-boundary.mjs` after changing service visibility or workspace consumers.

## Dependencies on Other RusToK Crates
- `rustok-content`
- `rustok-core`
- `rustok-media`
- `rustok-events`
- `rustok-outbox`
- `rustok-profiles`

## Common AI Mistakes
- Reconstructs category hierarchy from the flat compatibility list.
- Writes category placement/lifecycle/policy directly instead of owner commands.
- Stores category image URLs or reads Media tables.
- Parses mentions from code or unsanitized content.
- Resolves profiles by querying Profiles persistence directly.
- Emits mention delivery for unchanged targets or calls Notifications synchronously.
- Uses different identities for one outbox/journal event.
- Exposes handle snapshots, replay fingerprints or source body through relation reads.
- Updates immutable mention/quote rows instead of appending a relation revision.
- Accepts quote display text instead of typed target and quoted revision identity.
- Treats an omitted/empty quote set ambiguously; the D1 command is an explicit full replacement and empty clears.
- Lets REST/GraphQL import `MentionRelationService`, `PreparedMentionRelations` or `persist_in_tx`.
- Returns a quote command response through a post-commit read that can fail after the write has committed.
- Imports raw topic/reply implementation modules instead of root owner facades.

## Minimum Contract Set

### Input DTOs/Commands
- Public DTOs and commands exported from `src/lib.rs` define the input contract.
- Changes to existing create/update DTO fields are breaking and require synchronized callers; D1 therefore uses a separate quote replacement DTO.

### Domain Invariants
- All owner relations are tenant scoped and permission checked.
- Category hierarchy, policy and lifecycle remain bounded and database guarded.
- Mention extraction and profile resolution are bounded, format-aware and privacy fail-closed.
- Relation revisions and children are append-only and atomically matched to the persisted source body.
- Mention events share one identity across outbox and Forum journal.
- Relation reads and quote command responses are bounded and never expose private persistence fields.
- Quote replacement requires an exact existing source locale, deduplicates to at most 32 references and commits response materialization before commit.
- Quote targets retain target revision identity and cross-tenant/kind/target mismatches fail closed.
- Persistence modules and transaction seams are not transport API.

### Events / Outbox Side Effects
- Event publication uses the transactional outbox contract only.
- Event types and payload versions remain backward compatible.
- Mention events are emitted only for targets added by a newly persisted relation revision.
- Quote-only replacement does not synchronously call Notifications and does not emit events for unchanged mentions.

### Errors / Failure Codes
- Public `ForumError`/`ForumResult` preserve stable semantics across transports.
- Optional capability absence and provider failure remain distinct.
- Mention target failures use `FORUM_MENTION_TARGET_UNAVAILABLE`.
- Quote target failures use `FORUM_QUOTE_TARGET_UNAVAILABLE`.
- Invalid or foreign relation revision identities use `FORUM_RELATION_REVISION_UNAVAILABLE`.
