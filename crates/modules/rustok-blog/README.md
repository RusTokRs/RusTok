# rustok-blog

## Purpose

`rustok-blog` owns the Blog domain: posts, Blog Category membership/settings,
Blog-owned post-term relations, and comment integration through the neutral
`rustok-comments-api` contract implemented by `rustok-comments`.
Shared vocabulary and canonical Blog Category localized identity are provided by
`rustok-taxonomy` through explicit owner boundaries.

## Canonical native module reference layout

`rustok-blog` is the first enrolled reference implementation for the canonical
native module source layout defined by
[`docs/backend/module-backend-implementation.md`](../../../docs/backend/module-backend-implementation.md)
and the canonical layout ADR.

The physical source tree separates module wiring, domain policy, application
services, persistence, integrations, and transports without changing the public
Blog contract:

- `src/module.rs` owns `RusToKModule` / `MigrationSource` wiring;
- `src/domain/` owns the Blog state machine and article richtext policy;
- `src/services/` owns application/use-case orchestration and the `runtime`-gated
  module-work registrations (`src/services/scheduler.rs`);
- `src/entities/` and `src/migrations/` own Blog persistence;
- `src/integrations/` owns SEO, reaction, public-comment snapshot, and
  route/target-deletion adapters;
- `src/graphql/` and `src/controllers/` are thin owner transport adapters;
- `src/lib.rs` is a facade that preserves deliberate public re-exports.

Large responsibilities are nested below their owner slot. In particular,
`PostService` is split into command, query, repository/projection, helper-policy,
and test files instead of one shared source hotspot.
The preview-token HTTP lifecycle lives in `src/controllers/preview_token.rs`.
Shared HTTP authorization lives in `src/controllers/authorization.rs` and
the router and OpenAPI document both reference the capability handlers directly.

The baseline Blog migration keeps the **single registered**
`m20261029_000001_create_blog_schema` identity. Its responsibilities are
split below `src/migrations/m20261029_000001_create_blog_schema/schema/`:
ordered table builders, PostgreSQL constraints and SQLite triggers. No
new migration identity or upgrade/compatibility path is introduced. Tag
attachments (`blog_post_tags`) and their usage projection
(`blog_tag_usage`) declare composite `(tenant_id, tag_id)` foreign keys to
`taxonomy_terms(tenant_id, id)` with `ON DELETE CASCADE` in their shared
table builders, so term deletion enforces the same tenant-scoped cleanup on
PostgreSQL and SQLite.

The source shape is guarded by `npm run verify:module-source-layout`. That profile
admits only `lib.rs` and `module.rs` at the source root, so Blog has no root
`runtime.rs`: a runtime handle bundle belongs in `src/module.rs` or an existing
responsibility slot, which is why the `runtime`-gated
module-work registrations live in `src/services/scheduler.rs`.

## Canonical reference semantic contract

Blog is not a reference merely because its folders are canonical. It is also the
first implementation of the semantic reference contract in
[`DECISIONS/2026-09-18-canonical-native-module-reference-contract.md`](../../../DECISIONS/2026-09-18-canonical-native-module-reference-contract.md).

Key invariants include current-tenant authority, truthful locale provenance,
explicit `Patch<T>` updates, mandatory edit CAS, one executable lifecycle
policy, derived-state/business-revision separation, redacted public errors,
private persistence entities (with zero leakage; the orchestration bridge executes
via typed in-transaction owner commands in `services::orchestration` re-exported by
`ports`, closing finding L-4), owner-service integration seams, deterministic pagination,
and honest FBA readiness.

The module-owned UI packages use a separate adapter profile:
`core/` for commands/presentation/tests, `transport/` for GraphQL/native
adapters, and `ui/` for render/controller components. Bundled UI locales are
interface-copy coverage and do not restrict Blog content locales.

Run both `npm run verify:module-source-layout` and
`npm run verify:module-reference-contract` before using Blog as a template.
The [annotated connection map](docs/README.md#reading-and-extending-the-reference)
identifies actual permission, transaction, event, Search, cache, provider and UI
seams, including the limits of the current Media example.

## Responsibilities

- Provide `BlogModule` metadata for the runtime registry.
- Own Blog post lifecycle, Blog Category membership/settings/revision, SEO, and
  Blog-local orchestration.
- Own Blog GraphQL and REST transport adapters alongside domain services,
  including comment moderation and category CRUD under `/api/blog/categories`.
- Keep REST handlers on narrow `BlogHttpRuntime` state; the manifest-declared
  Axum router builds it from `HostRuntimeContext` and the host transactional
  event bus.
- Publish module-owned Leptos admin/storefront packages for installable UI
  surfaces.
- On both Next and Leptos storefronts, a nonempty `?q=` selects the Search
  owner's published Blog corpus rather than substring-filtering a Blog feed
  page. `category` UUID and exact localized/canonical `tag` are applied in
  Search SQL before its nine-result relevance/offset pagination; `?page=` is
  consistent across frameworks. With no query, Blog retains its own
  pinned-post keyset cursor. The indexed Blog document is an eventual read
  model and the live Blog Search audience port rechecks publication/channel.
- Keep Blog configuration limited to settings that have a canonical runtime owner;
  unused manifest-only settings are not declared.
- Publish separate typed RBAC resources: `blog_posts:*`,
  `blog_categories:*`, and the Blog-owned `tags:*` permission surface.
- Keep Blog Category commands synchronized with canonical Taxonomy Category
  state without restoring retired Blog Category translation storage.

## Category Taxonomy boundary

The former Blog-owned Category Translation target has been retired.
`BlogCategoryTranslationTargetProvider`, its owner change journal, its live
translation mirror and its provider-era test/evidence sources are not production
entry points.

The current boundary is:

- `rustok-taxonomy` owns canonical Blog Category localized copy and route
  history;
- Blog Category reads and mutation responses project canonical Taxonomy state;
- Blog Category create/update and hierarchy commands synchronize Taxonomy in the
  owner transaction;
- Blog Category delete delegates canonical lifecycle cleanup to Taxonomy;
- `blog_categories` remains Blog-owned for module membership, settings, owner
  revision and local command invariants;
- historical migration `000020` may read the donor translation table through a
  migration-local entity during upgrade, after which `000021` irreversibly removes
  `blog_category_translations` and `blog_translation_changes` once same-ID
  Taxonomy ownership is proven.

Do not reintroduce a second `blog/category` Translation provider or direct
localized Blog Category storage. Translation-control-plane work for Category
copy must use the canonical Taxonomy owner contract.

## Interactions

- Depends on `rustok-channel` for channel-aware public Blog read gating.
- Depends on `rustok-content` for shared content helpers and cross-domain
  orchestration primitives.
- Treat `rustok-comments` as an optional capability provider. Blog publication,
  category, tag, and post serving do not depend on Comments being enabled.
- Blog comment writes consume `RichTextDocument`; moderation reads consume the
  Comments-owned `RichTextView` and plain-text projection.
- Blog article writes accept the shared `RichTextDocument`; the owner applies
  the fixed `article` profile and persists canonical root JSON.
- Routes comment reads, create/update/delete, and moderation through the neutral
  `CommentsThreadPort` contract from `rustok-comments-api`; Blog does not import
  or call `CommentsService` directly.
- A generic `comments:create` grant never permits comments on drafts, archived
  or channel-hidden posts or bypasses the Blog comments mode: public REST and
  GraphQL writes apply the Blog publication/channel/policy gate. The separate
  owner create command requires `comments:moderate`, and both flows recheck
  the target after the external owner write; if it disappeared, they issue an
  idempotent compensating delete.
- A generic `comments:list` grant serves only Comments' approved public
  projection after rechecking Blog mode and channel visibility; the full
  pending/spam/trash queue requires `comments:moderate`. Direct comment
  details are available to a moderator or to the comment's own author;
  other readers see only approved comments on currently public posts.
- The static `blog -> comments` lifecycle edge is removed. Blog owns comment-surface
  policy; Comments owns threads, bodies, moderation, and retention. Provider absence
  fails comment operations closed while Blog publication serving remains available.
  Public comment reads revalidate post/channel visibility and the current Blog
  discussion mode after the Comments owner response. Cached approved-comment
  snapshots are never returned after the discussion becomes disabled, even if
  the mode changes during an upstream timeout.
- The projected `comment_count` on a post counts **approved comments only** — the
  number a public reader can actually see. Creation is moderation-pending and does
  not move the counter; approval moves it exactly once; moving a comment to
  spam/trash or deleting it moves it back; body edits never change it. The
  moderation queue reports its own totals separately and is not this counter.
- Reactions remains an optional integration. Blog owns whether its post surface
  requests reactions; the Reactions owner controls reaction capabilities and state,
  and effective availability must be resolved through an owner contract rather than
  by reading another module's lifecycle row.
- Profiles is optional presentation enrichment. Blog consumes the neutral
  `rustok-profiles-api` reader contract through host-composed GraphQL runtime data.
  When the provider is not composed or becomes unavailable, Blog returns posts with
  `author_profile = null` and never queries Profiles-owned tables directly.
- Depends on `rustok-taxonomy` for the shared tag dictionary and canonical Blog
  Category copy/hierarchy projection while keeping `blog_post_tags` Blog-owned.
  Global Taxonomy tags may be reused by Blog reads and attachments, but Blog tag
  mutation commands are restricted to `module:blog` terms; shared vocabulary remains
  Taxonomy-owned.
- Blog-owned `blog_tag_usage` is a synchronous derived read projection of
  `blog_post_tags`. It keeps tag-list pagination database-bounded, preserves
  `use_count DESC, canonical_key ASC, tag_id ASC` ordering, keeps zero-use
  module-local tags visible, and removes zero-use global tags. Taxonomy remains
  canonical for term identity and `canonical_key`; the projection is never an
  independent write source.
- Depends on `rustok-core` for module contracts, permissions, and
  `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter
  contracts.
- Used by `apps/server` through generated GraphQL composition and a
  manifest-declared Axum router mount.
- Used by `apps/admin` and `apps/storefront` through manifest-driven Leptos
  package composition.
- Search projection is a separate, asynchronously updated read model:
  `BlogSearchProjectionSource` emits only published, timestamped post
  translations with title, full article body, tags, category, author and
  `facets.category_id`; Search owns storage and PostgreSQL ranking. The host
  declares Blog as a **gated** Search audience. Blog authorizes each bounded
  candidate batch against the current tenant, post status, post-channel
  allowlist and Channel's active Blog-module binding rather than trusting the
  potentially stale search payload. Channel/tenant module disable and
  publication changes therefore hide results before the index catches up.
- Public Blog reads honor channel module bindings and typed post visibility
  allowlists. **Authentication alone does not bypass** that gate: only a
  tenant-wide editor may read posts independently of storefront channels.
  REST and GraphQL authenticated post lists apply public channel filtering in
  the owner query before cursor/limit calculation for readers without editor
  authority; direct unscoped public reads see globally visible posts only.
- Post adapters validate `blog_posts:*`; category adapters validate only
  `blog_categories:*`. Catalog `categories:*` and `blog_posts:*` do not
  authorize Blog Category operations.
- Blog services re-validate RBAC locally.
- `CategoryService::new(db, event_bus)` is the Category service constructor; the
  required `TransactionalEventBus` keeps owner mutation and Search reindex
  publication in the same transaction.

## Publication and URL contract

- Post slugs are global canonical identifiers, not locale-aware. They are
  normalized through the shared Taxonomy route-key primitive, so non-ASCII titles
  are transliterated.
- Blog derives a post's canonical route from its slug through
  `canonical_post_route` (`/modules/blog?slug={slug}`). `BlogPostRouteOwner`
  owns retired-route redirects in `blog_post_routes`. Renaming a post keeps the
  previous route as a redirect. Public reads resolve it to the current post, and the
  storefront redirects permanently to the current slug. Creating a post may take
  a retired slug. Deleting a post purges its routes.
- Route cleanup does not depend on that delete path: Blog consumes `TargetDeleted` for
  `blog_post` targets (`blog_target_deletion_routes`) and removes the matching routes idempotently,
  reporting them through `UrlAliasPurged`. A route whose target is gone no longer resolves — the
  read returns "not found" and records the `dangling_route_redirect` module metric — and the
  `blog_route_reconciliation` module-work registration sweeps the remaining rows in bounded,
  tenant-scoped batches.
- Route event metadata uses `CANONICAL_POST_ROUTE_LOCALE` (the platform fallback
  locale). Blog slugs and persisted redirect sources are locale-neutral.
- `published_at` is the first publication time. It is preserved across
  unpublish, archive, and restore. `updated_at` tracks the last change.
- `featured_image_url` must be an absolute `http`/`https` URL or a root-relative
  path, at most 2048 characters.
- Public reads use the requested locale with the fallback chain
  `requested -> explicit fallback -> en -> lexicographically smallest available`
  (the final fallback is deterministic; it cannot depend on query order).
- Translations are stored one row per `(post, locale)` under a unique index.
  A locale is the platform-normalized, extension-free BCP-47 identity accepted
  by `TenantLocale` — up to 32 characters (`rustok_api::locale::MAX_LOCALE_TAG_LEN`).
  `m20261030_000001_widen_blog_translation_columns` aligned the PostgreSQL
  `locale`, `title` and `seo_description` column widths with that contract
  (512-char titles, 1000-char SEO descriptions); SQLite never enforced the old
  widths. Removing a single locale's translation is not part of the owner
  contract: a post keeps every locale it has been given, and adding a new
  locale requires both a title and content in that locale.

See [`DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`](../../../DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md).

## Scheduled publishing

- Blog owns four durable work queues and publishes them to the host scheduler as
  `ModuleWorkRegistrations`; there is no module-local timer. The registration is
  `runtime`-gated in `src/module.rs`, the adapters live in `src/services/scheduler.rs`, and the
  host owns the poll loop and the deployment stop signal: `blog_scheduled_publish`,
  `blog_preview_token_cleanup`, `blog_route_reconciliation` and
  `blog_comment_projection_retention`. A deployment that does not
  run the host's module-work scheduler in `full`/`worker` mode gets no scheduled publishing.
- Every work item is tenant-scoped. The first discovery chooses the oldest due
  schedule; later local claims rotate over due tenant ids, even if an earlier
  tenant has a permanently failing draft. This fairness cursor is an
  in-memory scheduling hint, **not** durable retry state or a lease; other host
  instances may claim the same tenant. The authoritative owner command scans
  due drafts and uses post-version CAS to reject concurrent edits.
- `PostService::publish_scheduled_posts(tenant_id, limit)` publishes at most
  `limit.clamp(1, MAX_SCHEDULED_PUBLISH_BATCH)` = 200 **successful** due
  drafts per call. Database reads are individually bounded and seek forward by
  `(scheduled_at, id)`; a failing draft is logged, left due for repair/retry,
  and does not hide healthy drafts behind the first fixed-size page. Because
  failures do not count against the success budget, a run with many damaged
  rows can inspect more than 200 candidates. Each post runs the same versioned
  transition as manual publishing in its own transaction, with
  `BlogPostPublished` atomically announced under the system actor. The owner
  command is not reachable from an end-user transport.
- `scheduled_at` is a draft-only intent on both write paths. On create, setting it together
  with `publish: true` is rejected; on update, a `Set` patch is refused unless the row is a
  draft, and a past timestamp is refused on both. Publishing clears the schedule it consumed,
  so a published post never carries one.
- Setting a schedule announces `BlogPostScheduled` in the same transaction (on create and on a
  `Set` patch); clearing it announces `BlogPostScheduleCancelled`. A `Keep` patch publishes
  neither event. A schedule-only update publishes no content event at all: the search corpus
  holds published posts only, so neither `BlogPostUpdated` nor a reindex request accompanies
  a pure scheduling change.
- `blog_preview_token_cleanup` deletes the expired preview tokens of one tenant, at most
  `bounded_cleanup_batch(limit)` rows per iteration (clamped into
  `1..=MAX_PREVIEW_TOKEN_CLEANUP_BATCH`, i.e. 500 — `0` means one row, never the whole backlog).
- `blog_route_reconciliation` removes at most 200 stale routes per iteration whose
  `blog_post` target no longer exists, and reports them through `UrlAliasPurged`. It is the
  safety net for a missed `TargetDeleted` delivery, so it complements the idempotent
  `blog_target_deletion_routes` handler rather than replacing it.
- `blog_comment_projection_retention` compacts the Comments projection delivery ledger
  (`blog_comment_projection_deliveries`): for each comment it deletes the deliveries that
  are at least 30 days old **and** sit strictly below that comment's newest delivery, at
  most 200 rows per tenant iteration. The newest delivery is the only row the projection
  ever reads — the ULID idempotency frontier and the carried delta state — and a post's
  maximum `projection_revision` always lives on one of those frontier rows, so compacting
  everything below them is exactly lossless, and deleting a whole post still cascades its
  ledger rows away.

## Preview tokens

- `PreviewTokenService` mints an opaque 256-bit token (`URL_SAFE_NO_PAD` base64) for one post of
  one tenant. Only the token's SHA-256 digest is persisted; the plaintext is returned exactly
  once, by the create command, and token listings carry identity and expiry only. Tokens minted
  before the digest cutover no longer resolve and must be re-created. The REST read takes the
  token in the `x-blog-preview-token` header (never in the URL, which leaks into logs and
  browser history); the GraphQL `postByPreviewToken` argument is unchanged. A token read is
  unauthenticated by design: the token is the whole authorization of
  that read, which is why minting, expiry and revocation are strict. Brute-force protection
  is the 256 bits of entropy in the token (a guessing attempt resolves to "not found"), so
  the endpoint carries no per-token counter; deployments that want generic HTTP throttling
  place it at the host gateway, like every other public route.
- Minting is restricted to **drafts** and to the post author or a caller with post update
  permission. The TTL (`ttl_hours`, default 168) is validated against `1..=720`, so a token can
  neither be born expired nor outlive 30 days; an out-of-range request is refused instead of being
  stored.
- `get_post_by_token` resolves only an unexpired token whose tenant still has Blog enabled, and
  loads the post through the draft-only preview accessor. REST keeps its
  deliberately unscoped bearer-token contract. GraphQL uses
  `get_post_by_token_for_tenant`, which additionally restricts the lookup to the
  current authenticated/request tenant, so a foreign tenant's token cannot
  leak its draft through a different tenant's GraphQL endpoint. A revoked,
  expired, foreign (in GraphQL), or module-disabled token reads as "not found".
- `revoke_token` reports a missing or foreign token as `PreviewTokenNotFound`
  (`PREVIEW_TOKEN_NOT_FOUND`), never as a missing post.
- A token authorizes a read of a draft only, so leaving the draft state revokes the post's
  tokens inside the same lifecycle transaction: publishing (manually or through
  `blog_scheduled_publish`) and archiving both delete them. The accessor already refuses to
  serve a non-draft post, and revoking on the transition keeps a token row from coming back
  to life if the post returns to draft through `unpublish` or `restore`.
- Retention is tenant-scoped: `cleanup_expired(tenant_id, limit)` (limit clamped to
  `MAX_PREVIEW_TOKEN_CLEANUP_BATCH` = 500) is drained by the `blog_preview_token_cleanup`
  module-work registration of `src/services/scheduler.rs`, and deleting a post removes its tokens through
  the `fk_blog_preview_tokens_post` cascade. Like every Blog worker it runs only in host modes
  `full`/`worker`.

## Admin post listing

- `posts(filter:)` exposes the tenant's posts as keyset pages: `after`/`perPage` plus
  `sortBy` (`PUBLISHED_AT`, `UPDATED_AT`, `CREATED_AT`) and `sortOrder`
  (`ASC`/`DESC`), defaulting to `CREATED_AT DESC`. Each sort column has its own
  index from `m20261030_000002_add_blog_list_indexes`
  (`idx_blog_posts_tenant_created_id`, `idx_blog_posts_tenant_updated_id`,
  `idx_blog_posts_tenant_published_desc`, `idx_blog_posts_tenant_published_asc`), so a sorted page reads the index instead of sorting
  the table. The public listing's published-page scan rides the partial
  `idx_blog_posts_public_keyset` from the same migration.
- A cursor is bound to the sort it was issued for: reusing it with another
  `sortBy`/`sortOrder` is rejected with a validation error instead of resuming at a
  wrong position.
- The list never counts rows; `nextCursor` is present only while another page
  exists. NULL sort values come last in both directions.
- Both admin surfaces (the Leptos `rustok-blog-admin` table and the Next
  `@rustok/blog-admin` post list) expose that sort as a control. Changing the column or
  the direction clears `after` and the page history, because a cursor issued for one sort
  cannot be replayed against another.

## Bulk post operations

- Five GraphQL mutations act on a list of post ids — `bulkPublishPosts`,
  `bulkUnpublishPosts`, `bulkArchivePosts` (post publish permission), `bulkRestorePosts` and
  `bulkDeletePosts` (post delete permission). The same five operations exist over REST as
  `POST /api/blog/posts/bulk/{publish,unpublish,archive,restore,delete}` with the same
  permissions; both transports delegate to the same `PostService` bulk commands, and the
  request body is capped at 64 KiB by the host's JSON layer.
- The request is bounded at `MAX_BULK_POST_IDS` = 100 ids. The GraphQL input mirrors the
  bound with `#[graphql(validator(max_items = 100))]`, and
  `PostService::ensure_bulk_batch_is_bounded` re-checks it before any work starts, because a
  service caller is not a transport. An oversized request is refused as a validation error
  rather than truncated.
- A batch is **not** atomic across ids, and that is the contract: each id runs its own
  transition command in its own transaction with its own lifecycle event, and one id's
  failure never aborts the others. The response is per id — `succeeded`, `failed`,
  `succeededIds` and `failures { postId, code, message }` — so a caller can retry exactly the
  ids that failed.
- Every failure is reported through `BlogPublicError`, the module's single redacted public
  boundary: an `Invariant` or `Database` failure reaches the caller as the internal code and a
  generic message, never as display text naming storage internals.
- The fan-out is bounded from both sides: the 100-id cap and the per-root rate limit
  (`bulk_publish_posts`, `bulk_unpublish_posts`, `bulk_archive_posts`, `bulk_restore_posts`, `bulk_delete_posts` in
  the GraphQL rate-limit table), so one request is at most 100 transactions and 100 events.

## Known Limitations / Pending Implementation

Complete reference certification remains pending. Blog's shared test schema now
applies actual identity creation, OAuth prerequisites, Outbox, Content, Taxonomy,
all Channel migrations and Blog migrations on SQLite and PostgreSQL. The same
publication/channel/locale pagination scenario passes on both profiles.
Explicit PostgreSQL scenarios require `RUSTOK_BLOG_TEST_DATABASE_URL`; an ordinary
run reports them as ignored. Configured tests cover simultaneous edits, atomic
outbox rollback, duplicate/reordered delivery and connection/process restart.
These scoped results do not certify the full platform migrator or remote FBA.

Leptos Page Builder binds Blog-owned catalog, validation and preview transports.
Its shared editor still needs visual widget rendering, localized property labels
and Blog admission to the Page Builder FBA registry. Both Blog admin clients now
construct partial edits from the loaded target-locale snapshot and required
revision; acknowledged writes consume that snapshot before a separate reload.
Both storefronts compose reaction and comment surfaces. Scoped core, transport
and client checks do not establish mounted browser or accessibility parity. The
[reference acceptance plan](docs/implementation-plan.md#reference-readiness-review-2026-11-05)
records the findings, execution evidence, and the sequence before scaffold
extraction.

The following limitations are tracked in the module backlog:

- No server-side text search or facets for the post list.
- The rich-text body has no inline images, tables, or embeds.
- Readers cannot edit, delete, or report comments, and comments send no
  notifications.
- No trash or revision history. Draft preview exists through
  preview tokens (see "Preview tokens" above).
- Tag and category archives are query-string filters without their own pages.
- Anonymous REST read access is not available.
- Newsletter, membership, and paid content are not available.
- Reactions are limited to `like` on posts.
- Post creation has no stable logical command identity for replay after an
  uncertain network outcome. Confirmed acknowledgement blocks a duplicate UI
  submission, but retry after a lost acknowledgement is not exactly-once. The
  owner receipt/transport/client cutover and its failure scenario remain a
  reference acceptance gate.
- Preview reads have no audit trail;
  the token lifecycle itself is closed — a token of a post that left `draft` is revoked by the
  transition (see "Preview tokens").
- Bulk post operations run one transaction and one lifecycle event per id inside the request, so a
  batch is not atomic across ids and a request that dies mid-way leaves the remaining ids
  unprocessed; a queue-based bulk command that resumes is not implemented. The fan-out is bounded by the [100-id cap and the per-root rate limit](#bulk-post-operations).
- A route row whose post disappeared stays in the database until `blog_route_reconciliation` sweeps
  it, so the window is bounded by the host scheduler rather than by a cross-module transaction.
- The Leptos storefront does not redirect retired slugs.
- Blog Tag mutations are authorized by this module, not by the Taxonomy owner: the owner's
  `update_module_term_in_tx`/`delete_module_term_in_tx` accept a `SecurityContext` they do not
  read (`crates/modules/rustok-taxonomy/src/module_term_mutation.rs`), so
  `TagService::update_tag`/`delete_tag` enforce `Tags:Update`/`Tags:Delete` before the call and
  forward the same context. Adding `Resource::Taxonomy` enforcement to the owner is a
  cross-module policy decision (a role holding `Tags:Update` without `Taxonomy:Update` would
  stop being able to edit tags), tracked in
  [the Blog owner plan](./docs/implementation-plan.md).

## Entry points

- `BlogModule`
- `PostService`
- `CommentService`
- `CategoryService`
- `TagService`
- `graphql::BlogQuery`
- `graphql::BlogMutation`
- `controllers::axum_router`
- `admin::BlogAdmin`
- `storefront::BlogView`

## Docs

- [CRATE_API](./CRATE_API.md)
- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
