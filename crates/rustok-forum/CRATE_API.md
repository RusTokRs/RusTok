# rustok-forum / CRATE_API

## Public Modules
`category_presentation`, `constants`, `controllers`, `dto`, `entities`, `error`, `graphql`, `locale`, `services`.

## Primary Public Types and Signatures
- `pub struct ForumModule`
- `pub struct CategoryService`, `TopicService`, `ReplyService`, `ModerationService`, `SubscriptionService`, `UserStatsService`, `VoteService`
- `CategoryService::tree(tenant_id, security, CategoryTreeQuery) -> CategoryTreeResponse`
- `CategoryService::move_category(tenant_id, category_id, security, MoveCategoryInput) -> MoveCategoryResponse`
- `CategoryService::reorder_siblings(tenant_id, security, ReorderCategorySiblingsInput) -> ReorderCategorySiblingsResponse`
- `CategoryService::archive_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::restore_subtree(tenant_id, category_id, security) -> CategorySubtreeLifecycleResponse`
- `CategoryService::topic_policy(tenant_id, category_id, security) -> CategoryTopicPolicyResponse`
- `CategoryService::set_topic_policy(tenant_id, category_id, security, UpdateCategoryTopicPolicyInput) -> CategoryTopicPolicyResponse`
- `CategoryCoverMediaCandidate`, `normalize_category_icon_key`, `validate_category_cover_candidate`
- `pub mod graphql` -> `ForumQuery`, `ForumMutation`
- `pub mod controllers` -> `routes()`
- Public DTOs/constants from `dto::*` and `constants::*`
- `pub enum ForumError`, `pub type ForumResult<T>`
- `pub mod locale` — helpers `resolve_translation`, `resolve_body`, `available_locales`

## DTO changes (current)
### TopicResponse
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `slug: String`, `author_id: Option<Uuid>`, `vote_score: i32`, `current_user_vote: Option<i32>`, `is_subscribed: bool`, `solution_reply_id: Option<Uuid>`
### TopicListItem
- Added: `requested_locale: String`, `effective_locale: String`, `available_locales: Vec<String>`, `slug: String`, `author_id: Option<Uuid>`, `vote_score: i32`, `current_user_vote: Option<i32>`, `is_subscribed: bool`, `solution_reply_id: Option<Uuid>`
### ReplyResponse / ReplyListItem
- Added: `effective_locale: String`, `author_id: Option<Uuid>`, `parent_reply_id: Option<Uuid>` (in ListItem), `vote_score: i32`, `current_user_vote: Option<i32>`, `is_solution: bool`
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
### Category presentation contract
- Existing `icon` storage is interpreted as an `icon_key` and accepts only a bounded lowercase kebab-case semantic token at the database write boundary.
- Category colors remain bounded hexadecimal colors; CSS declarations and arbitrary color expressions are rejected.
- `CategoryCoverMediaCandidate` is the transport-neutral Media-to-Forum validation input and carries only media identity, tenant, MIME, size, dimensions and `MediaImageDescriptor`.
- `validate_category_cover_candidate` rejects foreign tenants, unsupported image MIME, oversized or dimensionless images, descriptor mismatch and non-direct-public delivery.
- Forum does not accept or store cover URLs, storage paths, drivers, credentials or blobs.
- Persistent `cover_media_id` writes remain disabled until the Media owner contract publishes quarantine/deletion state.
- Run `node scripts/verify/verify-forum-category-presentation.mjs` after changing this boundary.
### CreateTopicInput
- Added: `slug: Option<String>`
### ListRepliesFilter (new)
- Reply pagination: `page`, `per_page`, `locale`
### ModerationService
- Signatures `approve_reply`, `reject_reply`, `hide_reply`, `pin_topic`, `unpin_topic` now accept `tenant_id: Uuid`
- `close_topic`, `archive_topic` now accept `tenant_id: Uuid`
- Added `mark_solution(tenant_id, topic_id, reply_id, security)` and `clear_solution(tenant_id, topic_id, security)`
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

## Slug contract
- `CategoryResponse` / `CategoryListItem` return locale-aware slug at the
  `forum_category_translation` level; the slug follows the same resolved translation as
  `name` / `description`.
- `TopicResponse` / `TopicListItem` return a stable topic slug. When
  creating a new topic translation, the slug is copied from the seed-translation, unless
  a separate topic-level slug workflow is explicitly introduced.
- The current forum public contract remains ID-based: the forum API does not promise lookup
  by slug. If such a read-path is added later, it must use the
  same locale fallback contract as the rest of the forum read-path.

## Events
Publishes forum domain events through the outbox pipeline:
- `ForumTopicCreated` — when a topic is created
- `ForumTopicReplied` — when a reply is added
- `ForumTopicStatusChanged` — when topic status changes (close/archive)
- `ForumTopicPinned` — when topic is pinned/unpinned
- `ForumReplyStatusChanged` — when a reply is moderated (approve/reject/hide)

All new forum events are defined in `rustok-core::events::DomainEvent`.

## Owner Service Boundary
- Public topic and reply workflows use the root `TopicService` and `ReplyService` facade exports.
- Raw `services::topic`, `services::reply`, `topic_owner` and `reply_owner` implementations are crate-private.
- Public facades expose explicit create/read/update/list methods and never implement `Deref` to an implementation service.
- Topic/reply deletion must use facade `delete` methods so tombstones, counters and semantic events remain atomic.
- Run `node scripts/verify/verify-forum-owner-boundary.mjs` after changing topic/reply service visibility or workspace consumers.

## Dependencies on Other RusToK Crates
- `rustok-content`
- `rustok-core`
- `rustok-media` for transport-neutral image descriptors and candidate validation
- `rustok-outbox`

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
- Imports raw topic/reply implementation modules instead of the root owner facades.
- Passes methods to `ModerationService` without `tenant_id` — it is now required.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.
- Category tree reads fail closed for oversized, excessive-depth, untranslated, cyclic, disconnected, foreign-parent or invalid archive hierarchies.
- Category move/reorder commands use a per-tenant transaction order and never persist a partial sibling normalization.
- Category write paths enforce depth 16 at the database boundary; metadata updates cannot change sibling placement.
- Category subtree lifecycle is tenant-scoped, atomic, idempotent and enforced at the database boundary for category hierarchy and topic placement.
- Category topic policy is tenant-scoped and enforced at the database boundary for topic inserts and category reassignment.
- Category icon/color values are bounded safe tokens; cover media candidates are tenant-scoped and transport-neutral.
- Public topic/reply access is restricted to explicit owner facades; persistence modules and owner implementations are not part of the external contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
