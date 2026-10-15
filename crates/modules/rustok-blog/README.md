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
private persistence entities (the one documented exception is the
orchestration bridge surface re-exported by `ports`, pinned by
`verify:module-reference-contract` and tracked as L-4), owner-service integration
seams, deterministic pagination, and honest FBA readiness.

The module-owned UI packages use a separate adapter profile:
`core/` for commands/presentation/tests, `transport/` for GraphQL/native
adapters, and `ui/` for render/controller components. Bundled UI locales are
interface-copy coverage and do not restrict Blog content locales.

Run both `npm run verify:module-source-layout` and
`npm run verify:module-reference-contract` before using Blog as a template.

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
- The static `blog -> comments` lifecycle edge is removed. Blog owns comment-surface
  policy; Comments owns threads, bodies, moderation, and retention. Provider absence
  fails comment operations closed while Blog publication serving remains available.
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
- Public Blog reads honor channel module bindings and typed post visibility
  allowlists; authenticated/admin flows bypass the public channel gate.
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
- Post routes are owned by the canonical URL registry of `rustok-content`. Blog
  writes them through `CanonicalUrlWriter` and defines the route once in
  `canonical_post_route` (`/modules/blog?slug={slug}`). Renaming a post keeps the
  previous route as an alias. Public reads resolve it to the current post, and the
  storefront redirects permanently to the current slug. Creating a post may take
  a retired slug. Deleting a post purges its routes.
- Route cleanup does not depend on that delete path: Blog consumes `TargetDeleted` for
  `blog_post` targets (`blog_target_deletion_routes`) and removes the matching routes idempotently,
  reporting them through `UrlAliasPurged`. A route whose target is gone no longer resolves — the
  read returns "not found" and records the `dangling_route_redirect` module metric — and the
  `blog_route_reconciliation` module-work registration sweeps the remaining rows in bounded,
  tenant-scoped batches.
- Post routes are stored under `CANONICAL_POST_ROUTE_LOCALE` (the platform
  fallback locale), because Blog slugs are global canonical identifiers.
- `published_at` is the first publication time. It is preserved across
  unpublish, archive, and restore. `updated_at` tracks the last change.
- `featured_image_url` must be an absolute `http`/`https` URL or a root-relative
  path, at most 2048 characters.
- Public reads use the requested locale with the fallback chain
  `requested -> explicit fallback -> en -> first available`.

See [`DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md`](../../../DECISIONS/2026-10-08-blog-post-url-and-publication-contract.md).

## Scheduled publishing

- Blog owns three durable work queues and publishes them to the host scheduler as
  `ModuleWorkRegistrations`; there is no module-local timer. The registration is
  `runtime`-gated in `src/module.rs`, the adapters live in `src/services/scheduler.rs`, and the
  host owns the poll loop and the deployment stop signal: `blog_scheduled_publish`,
  `blog_preview_token_cleanup` and `blog_route_reconciliation`. A deployment that does not
  run the host's module-work scheduler in `full`/`worker` mode gets no scheduled publishing.
- Every work item is tenant-scoped. Discovery is a read-only hint (the tenant with the
  earliest due schedule) and the owner command re-checks the due predicate through the
  post-version CAS, so one tenant's backlog cannot starve another and a stale hint cannot
  publish the wrong post.
- `PostService::publish_scheduled_posts(tenant_id, limit)` publishes the drafts of one
  tenant whose `scheduled_at` has passed, oldest first, at most
  `limit.clamp(1, MAX_SCHEDULED_PUBLISH_BATCH)` = 200 per call. Each post runs the same
  transition command as a manual publish in its own transaction: a post edited after the scan
  fails the CAS and is skipped with a warning instead of being overwritten, and the next
  scheduler iteration picks it up. The `BlogPostPublished` event is published in the same
  transaction, under the system actor — which is why this command is never reachable from an
  end-user transport.
- `scheduled_at` is a draft-only intent on both write paths. On create, setting it together
  with `publish: true` is rejected; on update, a `Set` patch is refused unless the row is a
  draft, and a past timestamp is refused on both. Publishing clears the schedule it consumed,
  so a published post never carries one.
- Setting a schedule announces `BlogPostScheduled` in the same transaction (on create and on a
  `Set` patch); clearing it announces `BlogPostScheduleCancelled`. A `Keep` patch publishes
  neither event.
- `blog_preview_token_cleanup` deletes the expired preview tokens of one tenant, at most
  `bounded_cleanup_batch(limit)` rows per iteration (clamped into
  `1..=MAX_PREVIEW_TOKEN_CLEANUP_BATCH`, i.e. 500 — `0` means one row, never the whole backlog).
- `blog_route_reconciliation` removes at most 200 stale routes per iteration whose
  `blog_post` target no longer exists, and reports them through `UrlAliasPurged`. It is the
  safety net for a missed `TargetDeleted` delivery, so it complements the idempotent
  `blog_target_deletion_routes` handler rather than replacing it.

## Preview tokens

- `PreviewTokenService` mints an opaque 256-bit token (`URL_SAFE_NO_PAD` base64) for one post of
  one tenant. A token read is unauthenticated by design: the token is the whole authorization of
  that read, which is why minting, expiry and revocation are strict.
- Minting is restricted to **drafts** and to the post author or a caller with post update
  permission. The TTL (`ttl_hours`, default 168) is validated against `1..=720`, so a token can
  neither be born expired nor outlive 30 days; an out-of-range request is refused instead of being
  stored.
- `get_post_by_token` resolves only an unexpired token whose tenant still has Blog enabled, and
  loads the post through the draft-only preview accessor. A revoked, expired, foreign, or
  module-disabled token reads as "not found" rather than as an error.
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
  index from `m20260328_000001`, so a sorted page reads the index instead of sorting
  the table.
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

- Four GraphQL mutations act on a list of post ids — `bulkPublishPosts`,
  `bulkUnpublishPosts`, `bulkArchivePosts` (post publish permission) and `bulkDeletePosts`
  (post delete permission). No REST endpoint exposes bulk work.
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
  (`bulk_publish_posts`, `bulk_unpublish_posts`, `bulk_archive_posts`, `bulk_delete_posts` in
  the GraphQL rate-limit table), so one request is at most 100 transactions and 100 events.

## Known Limitations / Pending Implementation

The following are not implemented. Each is tracked with its audit identifier in
[the current implementation cursor](./docs/implementation-plan-current.md#open-backlog-known-limitations).

- No server-side text search or facets for the post list (H-7).
- The rich-text body has no inline images, tables, or embeds (H-8).
- Readers cannot edit, delete, or report comments, and comments send no
  notifications (H-9).
- No trash, draft preview, or revision history (M-3, M-4).
- Tag and category archives are query-string filters without their own pages (M-7).
- Anonymous REST read access is not available (M-8).
- Newsletter, membership, and paid content are not available (M-9).
- Reactions are limited to `like` on posts (M-10).
- Preview reads have no audit trail (M-13 of
  [`docs/audits/rustok-blog-engineering-audit-2026-10-15.md`](../../../docs/audits/rustok-blog-engineering-audit-2026-10-15.md));
  the token lifecycle itself is closed — a token of a post that left `draft` is revoked by the
  transition (see "Preview tokens").
- Bulk post operations run one transaction and one lifecycle event per id inside the request, so a
  batch is not atomic across ids and a request that dies mid-way leaves the remaining ids
  unprocessed; a queue-based bulk command that resumes is not implemented (H-5 of the 2026-10-15
  audit). The fan-out is bounded by the [100-id cap and the per-root rate limit](#bulk-post-operations).
- A route row whose post disappeared stays in the database until `blog_route_reconciliation` sweeps
  it, so the window is bounded by the host scheduler rather than by a cross-module transaction
  (H-7 of the 2026-10-15 audit).
- The Leptos storefront does not redirect retired slugs (M-12).
- Blog Tag mutations are authorized by this module, not by the Taxonomy owner: the owner's
  `update_module_term_in_tx`/`delete_module_term_in_tx` accept a `SecurityContext` they do not
  read (`crates/modules/rustok-taxonomy/src/module_term_mutation.rs`), so
  `TagService::update_tag`/`delete_tag` enforce `Tags:Update`/`Tags:Delete` before the call and
  forward the same context. Adding `Resource::Taxonomy` enforcement to the owner is a
  cross-module policy decision (a role holding `Tags:Update` without `Taxonomy:Update` would
  stop being able to edit tags), tracked in
  [the current implementation cursor](./docs/implementation-plan-current.md#open-backlog-known-limitations).

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

- [Module docs](./docs/README.md)
- [Current implementation cursor](./docs/implementation-plan-current.md)
- [Platform docs index](../../../docs/index.md)
