# RusTok Blog Module — Deep Engineering Audit

- **Date**: 2026-10-15
- **Repository**: `RusTokRs/RusTok`
- **Base revision audited**: `1a420c8ffc2dedb681ff8e9f45ee3bdb446321b5` (`main`), working tree clean at audit start
- **Subject**: `crates/modules/rustok-blog` (backend), `crates/modules/rustok-blog/admin` (Leptos admin package),
  `crates/modules/rustok-blog/storefront` (Leptos storefront package), `apps/next-admin/packages/blog`,
  `apps/next-frontend/packages/rustok-blog`, host wiring in `apps/server`, migrations, contracts, verifiers and docs
- **Method**: full manual source read of the module and its host integration, static cross-checks
  (entity ↔ migration parity, query ↔ index coverage, transport ↔ schema parity, docs ↔ source parity),
  execution of the Node contract verifiers that exist for Blog, and inspection of CI wiring.
- **Evidence language**: English only, per `AGENTS.md` §12.
- **Audit protocol**: per `AGENTS.md` §2.1. Base `main` revision recorded above; all work is on the
  session branch `arena/f1b01fb0-rustok`.

## 0. Hard evidence limits (read before using this document)

`cargo` and `rustc` are **not installed in the audit environment**. Therefore:

- **No compilation was performed.** No `cargo check`, `cargo test`, `cargo clippy`, `rustfmt --check` result is claimed.
- **No runtime evidence exists** for this audit: no PostgreSQL/SQLite run, no server boot, no GraphQL execution,
  no UI rendering.
- Everything below is **source-level** evidence: file/line citations, static invariants, and the outputs of the
  repository's own Node verifiers (which are text/AST-free string matchers).
- Per `AGENTS.md` §15, this document does **not** promote any module status, does not claim green gates, and
  does not claim that any previously documented "certified" state is verified. Where the repository documents
  "source verified, compile not run", that is quoted verbatim as a claim, not as evidence.

Known-good Node executions performed during this audit (node v22.22.3, from the repository root) are listed in
Appendix A. Where a verifier fails, the failure is classified as **script defect**, **stale expectation**, or
**real gap**, with reasoning.

---

## 1. Executive summary

The Blog module is a large, serious implementation: 148 Rust files / ~35,700 LOC in the module crate alone,
29 migrations, 9 SeaORM entities, a type-state + table state machine, a transactional outbox event surface,
keyset-paginated read paths, a Taxonomy-owned Category projection, an owner-scoped Comments port, RBAC with
`blog_posts:*` / `blog_categories:*` / `tags:*` namespaces, dual UI stacks, and ~67 Node contract verifiers.

The **architectural** claims in the canonical reference documentation are largely true in the source: owner-scoped
routes (`BlogPostRouteOwner`), fail-closed comment availability, redacted public errors, canonical Taxonomy
ownership of Category copy, Patch-based nullable update semantics, version CAS on lifecycle transitions,
idempotent comment-count projection.

However, this audit found that the **runtime completeness and the enforcement story are materially weaker than
the documentation implies**:

1. **Scheduled publishing has no executor.** `PostService::publish_scheduled_posts()` has no caller anywhere in
   the repository; no scheduler binary, cron entry, or worker exists; `BlogPostScheduled` has no consumer.
   The feature is documented as implemented.
2. **Public listing pagination is wrong when pinned posts exist.** The public keyset query orders by
   `is_pinned DESC, pinned_at DESC, published_at DESC, id DESC` while the cursor only encodes
   `(published_at, id)`. Pinned posts can be skipped or duplicated across pages.
3. **Rate limiting is fail-open for most of the Blog GraphQL surface**, including the public comment write path
   and the newest write paths (bulk, pin/unpin, preview tokens). Only 10 named root fields are classified.
4. **~59 of 67 Blog verifiers cannot run in CI** (no workflow references them), and at least 20 crash with a
   JavaScript `SyntaxError`. Two verifiers hold mutually contradictory expectations, and another two contradict
   `verify-module-reference-contract.mjs`. The "machine-gated canonical reference" claim is therefore only
   partially true.
5. **Several recently added capabilities ship without owner test coverage and without any UI**: pin/unpin, bulk
   operations, preview tokens; the Leptos admin transport even hard-codes `is_pinned: None`.

Net assessment: **Blog is an above-average reference-shaped module with a partially fictional completion story.**
The core post lifecycle, Category cutover, comment projection and RBAC are in good shape at the source level.
Scheduling, preview tokens, pinning, bulk operations, tag pagination, public-list pagination and the verifier
gate must be considered **incomplete** until the gaps in §3 are closed and compile/runtime evidence is produced.

Counts (this audit): **7 High**, **13 Medium**, **9 Low** findings; 4 governance/verification findings
(§4) plus 1 contradiction cluster.

---

## 2. Functional inventory (what actually exists)

This section describes the implemented functionality as read from source. Status words are
**implemented** (source present and coherent), **partial** (source present but incomplete/unwired),
and **absent**.

### 2.1 Module manifest, settings and runtime wiring

| Item | Location | Status | Notes |
|---|---|---|---|
| Module manifest | `crates/modules/rustok-blog/rustok-module.toml` | implemented | `slug = "blog"`, `ownership = "first_party"`, `trust_level = "verified"`, `ui_classification = "dual_surface"`, `recommended_admin_surfaces = ["leptos-admin"]`, `showcase_admin_surfaces = ["next-admin"]` |
| Module dependencies | same, `[dependencies]` | implemented | `content`, `outbox`, `taxonomy`, `channel`; matches `modules.toml:75` |
| Settings: `comments_mode` | same | implemented | `string`, default `open`, options `disabled`/`read_only`/`open`; consumed by `CommentService::public_comments_mode` (`src/services/comment.rs:66`) and the public snapshot (`src/integrations/public_comments_snapshot.rs:75`) |
| Settings: `use_reactions` | same | implemented | `boolean`, default `false`; read by `BlogReactionSubjectProvider` (`src/integrations/reaction_subject.rs:74-92`); disabled/invalid ⇒ `Unavailable` / `Internal` (fail closed) |
| Provides: GraphQL | same | implemented | `graphql::BlogQuery`, `graphql::BlogMutation`, `attach_schema_data` |
| Provides: HTTP | same | implemented | `controllers::axum_router` |
| Provides: admin UI | same | implemented | Leptos crate `rustok-blog-admin`, route segment `blog`, i18n `en`/`ru`, child pages `posts`, `new`, Next package `@rustok/blog-admin` |
| Provides: storefront UI | same | implemented | Leptos crate `rustok-blog-storefront`, slot `home_after_catalog`, Next package `@rustok/blog-frontend` |
| FBA consumer block | same, `[fba.consumer]` | partial | registry `contracts/blog-fba-registry.json`, profile `blog_post_comments`, provider contract `comments.thread.v1`, degraded modes `hide_comment_form`, `show_cached_thread_snapshot`; comments availability is fail-closed in source, but the registry documents `boundary_ready`, not executed evidence |
| Event listeners | `src/module.rs:96-101` | partial | exactly **one** listener is registered: `BlogCommentProjectionHandler`. Blog does not consume any event (see H-7) |
| Module tests | `src/module.rs:202-208` | implemented | asserts exactly one handler is registered |

### 2.2 Persistence and migrations

29 migrations (`src/migrations/m2026*.rs`), all registered in `src/migrations/mod.rs` in sorted order
(verified programmatically: 29 files, 0 missing, ordering monotonic). Numbering gaps at `000003-000006`,
`000012-000015` and `000030` are **not** blog gaps — those numbers are used by sibling modules
(e.g. `rustok-forum/.../m20260823_000030_*`, `rustok-product/.../m20260913_000030_*`).

Tables owned by Blog:

| Table | Created / changed by | Notes |
|---|---|---|
| `blog_posts` | `m20260328_000001`, then `000018`(indirect), `000023`, `000025`, `000031` (drop `view_count`), `000033` (`scheduled_at`), `000035` (`is_pinned`, `pinned_at`) | Keyset indexes for public and admin orderings; `version` optimistic-lock column; `comment_count` is a projection, not authoritative |
| `blog_post_translations` | `m20260328_000001`, `m20260721_000005` | FK to `blog_posts` `ON DELETE CASCADE`; unique `(post_id, locale)` |
| `blog_categories` | `m20260328_000002`, `000017` (tenant-parent FK + identity index), `000022` (drops `parent_id`/`position`/`depth`), `000028` (`post_count` removed), `000029` (settings CHECK + triggers) | Membership/settings only; hierarchy lives in Taxonomy |
| `blog_category_translations` **and** `blog_translation_changes` | created `000002`/`000016`, **dropped** by `m20260828_000021` | Retirement is irreversible by design (`down()` returns `Ok(())`) |
| `blog_post_tags` | `m20260328_000002`, `000018` (composite tenant FKs + triggers) | Attachment owned by Blog, term owned by Taxonomy |
| `blog_post_channel_visibility` | `m20260329_000001`, `000024` (tenant triggers) | Unique `(tenant, post, slug)` |
| `blog_tag_usage` | `m20260922_000027` | Projection with `canonical_key` + `use_count` |
| `blog_comment_projection_deliveries` | `m20260716_000001`, `m20260922_000026` (adds `projection_revision` + index) | Idempotency ledger for comment counts |
| `blog_comments_delegation_*` (schedule state, audit outbox, recovery, canonical handoff) | `000007`–`000011` | Append-only audit with immutability triggers |
| `blog_category_taxonomy_bindings` | `m20260824_000019`, cleaned by `m20260916_000022` | Legacy pre-cutover binding; the current read path no longer uses it (see M-9) |
| `blog_post_routes` | `m20261008_000032` | Redirects only; canonical Blog routes are derived and never stored |
| `blog_preview_tokens` | `m20261009_000034` | Unique token, expiry index, **no FK to `blog_posts`** (see M-1) |

Tenant integrity: cross-tenant integrity is enforced at the DB layer for `blog_post_tags` (`000018`),
`blog_posts.category_id` (`000023`/`000025`), and `blog_post_channel_visibility` (`000024`) with PostgreSQL
triggers plus SQLite trigger equivalents; Category settings shape/size is enforced by `000029`.

### 2.3 Domain layer

- `src/domain/state_machine.rs` (611 lines): type-state wrappers `BlogPost<Draft|Published|Archived>` plus the
  runtime table `BlogPostStatus::can_transition_to`, `Draft → Published`, `Published → Draft`,
  `Published → Archived`, `Archived → Draft`, and `CommentStatus` (`Approved`/`Spam`/`Trash`) for the comments
  integration. Property tests in `state_machine_proptest.rs` (31 test cases).
- `src/domain/richtext.rs` (170 lines): Article-profile normalization + projection to `RichTextView`/plain text;
  ReDoS/shape guards for stored documents.
- `src/domain/comment_policy.rs` (61 lines): `BLOG_COMMENTS_MODE_SETTING = "comments_mode"`,
  `parse_comments_mode` **fails closed** for unknown values (`Err(())` ⇒ caller maps to an internal error).

### 2.4 Services

**`PostService`** — `src/services/post/{mod,commands,queries,repository,routes,bulk,pin,tests}.rs`

- Create (`commands.rs`): validates `scheduled_at` (future, mutually exclusive with `publish`), normalizes
  locale, normalizes/claims the canonical slug (releasing a retired route if needed), validates/creates tags
  through Taxonomy, enforces the reserved-metadata contract, writes post + translation + channel visibility +
  tags in one transaction, publishes `BlogPostCreated` (+ `BlogPostPublished`/`BlogPostScheduled`).
- Update: `Patch<T>` semantics for `excerpt`/`category_id`/`featured_image_url`/`seo_*`, mandatory `version`
  CAS, slug rename handling (`release_slug_route_in_tx` + `redirect_retired_slug_in_tx`), channel visibility
  replacement, tag re-sync, localized-copy provenance rules, `ReindexRequested` on content changes.
- Lifecycle: `publish_post`, `unpublish_post`, `archive_post` (bounded reason), `restore_post`
  (Archived → Draft), all with `enforce_scope` + `enforce_owned_scope`, transition validation,
  `version` CAS via `apply_status_transition_in_tx` (rejects `rows_affected != 1` with a conflict), and
  domain events in the same transaction.
- Delete: only Draft/Archived (published posts are refused with `CannotDeletePublished`); deletes tag usage,
  post row (version CAS), redirect rows, then publishes `BlogPostDeleted` + `TargetDeleted`.
- Read paths (`queries.rs`, `repository.rs`): `get_post_with_locale_fallback` (fails closed on missing
  translation sets), `list_posts_with_locale_fallback` (admin keyset on sort column + `id`),
  `list_public_visible_keyset` (public cursor pagination + channel visibility),
  `get_posts_by_tag/category/author` (admin wrappers), `scan_published_posts` (sitemap/bulk scan; keyset by
  `id`, no channel filter), `summarize_posts` (batched translations/channels/tags/category names).
- Routes: `BlogPostRouteOwner` implements the ADR-2026-10-08 owner-route model — `find_redirect`,
  `release_slug_route_in_tx`, `redirect_retired_slug_in_tx`, `remove_post_routes_in_tx`, emitting
  `CanonicalUrlChanged` / `UrlAliasPurged`. Canonical route is derived from the slug
  (`canonical_post_route`, `/modules/blog?slug=...`), never stored.
- Bulk (`bulk.rs`, 67 lines): `bulk_transition_posts` (publish/unpublish/archive) and `bulk_delete_posts`,
  iterating ids and collecting per-post successes/failures.
- Pin (`pin.rs`, 74 lines): `pin_post`/`unpin_post` ⇒ `set_pinned`, `blog_posts:update` + owned scope,
  published-only pinning.

**`CategoryService` / `CategoryCommandService`**

- `category_owner.rs` exposes the Taxonomy-owned Category reader facade (`create/get/update/delete/list`);
  `category.rs` holds the Blog-owned write core (membership + settings) that syncs canonical copy to Taxonomy
  in the same transaction (`category_taxonomy_sync.rs`), serializes with
  `lock_category_hierarchy_writer_in_tx`, enforces tree capacity (`MAX_BLOG_CATEGORY_TREE_NODES`), and validates
  settings (JSON object, ≤ 64 KiB).
- `category_command.rs`: `move_category` validates the full canonical Taxonomy placement projection, computes
  old/desired depths, persists sibling order and descendant depth changes, rejecting cycles/unbounded trees.
- `category_delete.rs`: Blog-side cleanup (detach category from posts, remove membership) invoked through the
  Taxonomy delete hook.
- `category_name_projection.rs`: loads Category names **from the canonical Taxonomy projection**
  (`TaxonomyOwnerCategoryReader::load_scoped_categories_strict`) with coverage/locale invariants.

**`TagService`** (`tag.rs`, 903 lines)

- Create/update/delete delegate to `TaxonomyService::*_module_term_in_tx` inside a Blog transaction, with
  `initialize_tag_usage_in_tx` and `publish_blog_reindex_in_tx` in the same transaction.
- Delete additionally bumps `version`/`updated_at` for every post that loses the tag
  (`bump_posts_for_tag_relation_removal_in_tx`) and fails with `conflict` if the affected row count drifts.
- `sync_post_tags_in_tx` rewrites `blog_post_tags` and maintains `blog_tag_usage`
  (`increment_tag_usage_in_tx` upsert / `decrement_tag_usage_in_tx` guarded update).
- `list_tags` (page/per_page, `use_count DESC, canonical_key ASC, tag_id ASC`) joins the usage projection with
  Taxonomy terms and errors with `invariant` if the projection references a missing / non-Blog module term.

**`CommentService`** (`comment.rs`, 843 lines)

- Optional `CommentsThreadPort` + `SharedStaticModuleSettingsReader`; public operations fail closed
  (`CommentsPolicyUnavailable` when no settings snapshot can be read).
- `create_public_comment` = scope + mode + post visibility check, then writes through the port, then
  **re-validates** and compensates with an idempotent cross-owner delete under `SecurityContext::system()` and a
  fresh command id; a failing compensation returns `BlogError::invariant`.
- `update_comment` / `moderate_comment` / `delete_comment` pre-read through `system()` before
  `ensure_blog_target`.
- Public listing bypasses `Action::List` but returns `CommentsDisabled` when the mode forbids reading.

**`BlogCommentProjectionHandler`** (`comment_projection.rs`): per-post `lock_exclusive`, delivery ledger keyed on
`event_id`, monotonic `projection_revision`, per-comment event ordering, `+1`/`-1` applied delta only for
`Created`/`Deleted` lifecycle states, saturating non-negative `comment_count`, no mutation of Blog business
`version`/`updated_at`, `ReindexRequested` only when the count changed, deleted posts drop delivery rows.

**`PreviewTokenService`** (`preview_token.rs`, 228 lines): `create_token` (TTL default 168h, capped at 720h),
`get_post_by_token` (expiry check, then loads the post with `SecurityContext::system()`),
`revoke_token`, `list_tokens`, `cleanup_expired`.

**RBAC** (`rbac.rs`, 81 lines): `enforce_scope` + `enforce_owned_scope`, one Blog permission namespace
(`BlogPosts`, `BlogCategories`, `Tags`, `Comments`) with no privileged `Posts` fallback; the module source
verifier asserts the ordering of scope checks before reads in the command paths.

### 2.5 GraphQL surface (`src/graphql/`)

Root query fields (verified from the object impl): `post`, `postBySlug`, `posts`, `publicBlogPosts`,
`blogCategory`, `blogCategories`, `blogTags`, `postByPreviewToken`.
Root mutation fields: `createPost`, `updatePost`, `deletePost`, `publishPost`, `unpublishPost`, `archivePost`,
`restorePost`, `createBlogComment`, `moderateComment`, `createBlogCategory`, `updateBlogCategory`,
`deleteBlogCategory`, `createPreviewToken`, `revokePreviewToken`, `bulkPublishPosts`, `bulkUnpublishPosts`,
`bulkArchivePosts`, `bulkDeletePosts`, `pinPost`, `unpinPost`.

Behavioural properties read from source:

- Every root begins with `require_module_enabled(ctx, "blog")`; public reads additionally require a public Blog
  channel binding (`require_public_blog_channel_enabled`); `posts` rejects public callers explicitly
  ("Public blog listings use publicBlogPosts with cursor pagination").
- `GqlPostStatus` is exported as `BlogPostStatus` with `SCREAMING_SNAKE_CASE` items (matches the Next.js client
  union `'DRAFT' | 'PUBLISHED' | 'ARCHIVED'`).
- `GqlPost` carries `is_pinned`/`pinned_at`; `GqlPostListItem` carries `is_pinned`, `pinned_at`, `tags`,
  `channel_slugs`, `category_name`, `author_profile`.
- `PostsFilter` exposes `status`, `category_id`, `tag`, `author_id`, `locale`, `after`, `per_page` — **no sort
  control** (the resolver hard-codes `CreatedAt`/`Desc`, see M-12).
- `BulkOperationInput.post_ids: Vec<Uuid>` with **no validator bound** (see H-5).
- `GqlBulkOperationFailure.reason` is a raw `String` copied from `BlogError::to_string()` (see H-5).
- Rate limiting (`rate_limit.rs`, 494 lines): a request-level extension classifying the document into one of 10
  `BlogGraphqlSurface` values, tenant/actor-scoped keys, `Retry-After`, pluggable limiter trait. Fields outside
  the enum are not limited at all (see H-4).

### 2.6 REST surface (`src/controllers/`)

Registered routes (from `#[utoipa::path]` attributes):

- `/api/blog/posts` (GET list, POST create), `/api/blog/posts/{id}` (GET, PUT, DELETE),
  `.../publish`, `.../unpublish`, `.../archive`, `.../restore`
- `/api/blog/preview/{token}` (GET, **unauthenticated**) — see H-3
- `/api/blog/posts/{id}/pin`, `/api/blog/posts/{id}/unpin`
- `/api/blog/categories` (GET/POST), `/api/blog/categories/{id}` (GET/PUT/DELETE), `.../move`
- `/api/blog/comments/{id}/moderate`

`controllers/mod.rs` builds a narrow `BlogHttpRuntime` from `HostRuntimeContext`; every handler except the
preview handler calls `ensure_blog_module_enabled(&runtime, tenant.id)` and the tenant-binding permission guard
(`blog_tenant_mismatch` / `blog_category_tenant_mismatch`).

`controllers/openapi.rs` publishes a `utoipa` document that lists **16 handlers** and **omits**
`preview_post`, `pin_post`, `unpin_post` (see M-6).

### 2.7 UI packages

- **Leptos admin** (`crates/modules/rustok-blog/admin`, 3,725 LOC): module root `BlogAdmin` = post editor +
  moderation panel; `core/{commands,presentation,tests}` (386/851/721 lines), `transport/{native_server_adapter,
  graphql_adapter,moderation_adapter}`, `ui/{components,leptos,richtext}`, FTL locales `en`/`ru`.
  Moderation slice is separated so editors without `blog_posts:manage` keep CRUD; missing GraphQL contracts
  degrade to an unavailable list/moderation panel instead of a dashboard error. `model.rs` carries
  `scheduledAt`, `isPinned`, `pinnedAt`. **No command, UI control, or adapter call emits `isPinned`** — the
  GraphQL adapter hard-codes `is_pinned: None` (`graphql_adapter.rs:224,258`) and the native adapter does the
  same (`native_server_adapter.rs:304,367`). No pin/unpin/bulk/preview-token UI exists.
- **Leptos storefront** (`crates/modules/rustok-blog/storefront`): core + transport + `ui/{leptos,comment,
  comments_list,share}`, cursors/formatting helpers, 62 core tests. No pin affordance; pinned ordering is
  inherited from the backend ordering only.
- **Next.js admin package** (`apps/next-admin/packages/blog`): `api/posts.ts` (318 lines), `api/categories.ts`,
  `post-form.tsx`, `post-table/*`, `pages/*`. Its GraphQL documents match the current schema
  (`tenantId` argument, `BlogPostStatus` screaming-snake enum). The `PostSummary.status` union is
  `'DRAFT' | 'PUBLISHED' | 'ARCHIVED'`; the list query requests `scheduledAt` twice (harmless duplicate
  selection, but a lint/quality defect). No pin token/bulk scheduling UI.
- **Next.js storefront package** (`apps/next-frontend/packages/rustok-blog`): `fetchPublishedPost(s)`,
  post card, comments section/composer/pagination, threaded comments, reaction bar, share buttons, TOC,
  reading time, JSON-LD helpers; the app routes are `/[locale]/blog`, `/[locale]/blog/[slug]`,
  `/[locale]/blog/feed.xml`. The RSS feed sets a 25-item cap and applies the backend locale fallback chain.
- `crates/modules/rustok-blog/build.rs` (feature `comment-assets`): copies the comment bootstrap script into
  `target/site/assets/...` and writes stub JS/WASM when the built artifacts are missing.

### 2.8 Integrations

- **SEO** (`integrations/seo_targets.rs`, 344 lines): `BlogSeoTargetProvider` implements `load_target`,
  `resolve_route`, `sitemap_candidates`, `list_bulk_summaries_page`; canonical `/modules/blog?slug=...` and
  hreflang per available locale; redirect data comes from `blog_post_routes`.
- **Reactions** (`integrations/reaction_subject.rs`, 230 lines): provider factory + catalog (`like`, single
  selection), `use_reactions` gate, post must be `Published` and channel-visible, revision must match the
  persisted `version`, `Conflict` on mismatch, `deletion_bindings` maps reactions to `blog_post`.
- **Public comments snapshot** (`integrations/public_comments_snapshot.rs`, 519 lines): snapshot schema v3,
  256 KiB cap, cache identity = tenant/post/locales/channel/page/per_page/projection revision, availability
  `Available | Disabled | ReadOnly | Unavailable | Timeout`, page ≥ 1, `per_page ∈ 1..=100`, cache stores
  plain-text previews (200 chars) and never leaks unapproved bodies.
- **Search** (`crates/modules/rustok-search/src/blog_projector.rs` + `ingestion.rs`): projectors for
  rebuild/upsert/author refresh; ingestion handles `BlogPostCreated/Published/Unpublished/Updated/Archived/
  Deleted` (delete maps to `delete_blog_post`). `BlogPostScheduled`/`BlogPostScheduleCancelled` are **not**
  handled by ingestion (informational only).
- **Comments port**: Blog consumes `rustok_comments_api::CommentsThreadPort`; the host publishes either the
  in-process provider (`apps/server/src/services/mod.rs:168`) or a TCP remote adapter
  (`comments_provider_runtime_base.rs` chain reachable via
  `comments_provider_runtime → keyring_schedule_trigger_guard → … → keyring → base`).

### 2.9 Events and cross-module contracts

Published by Blog: `BlogPostCreated`, `BlogPostUpdated`, `BlogPostPublished`, `BlogPostUnpublished`,
`BlogPostArchived`, `BlogPostDeleted`, `BlogPostScheduled`, `BlogPostScheduleCancelled`,
`ReindexRequested` (`target_type = "blog"`), `TargetDeleted { target_type: "blog_post" }`,
`CanonicalUrlChanged`, `UrlAliasPurged`.

Consumed by Blog: `CommentCreated` / `CommentDeleted` / `CommentStatusChanged` / `CommentUpdated`
(via `BlogCommentProjectionHandler`) — and nothing else. In particular Blog does **not** consume
`TargetDeleted`, despite the ADR requiring redirect-holding owners to purge rows from that event (see H-7).

`CanonicalUrlChanged` is consumed by `rustok-seo/src/services/canonical_redirect_handler.rs` (real consumer).
`UrlAliasPurged` has **no consumer** outside event validation/tests in this revision (SEO handles only
`CanonicalUrlChanged`), so purge/retry semantics described by the ADR are not implemented anywhere.

### 2.10 Tests, contracts and verifiers

- Rust tests present: `tests/integration.rs` (61 test fns), comment projection PG suites (31 + 20 + 19 + 15),
  category taxonomy cutover suites (`category_taxonomy_delete_lifecycle`, `..._structure_sync`,
  `..._dual_write`, `..._read_cutover`, `..._mutation_response_cutover`, `..._translation_provider_retirement`),
  `category_hierarchy`, `category_settings_contract`, `graphql_*`, `taxonomy_tags`, `module`, `contract_surface`,
  plus in-crate unit/property tests.
- Contracts: `contracts/*.json` + ~50 `contracts/evidence/*.json` files describing source-verified contracts.
- ~67 Node verifiers in `scripts/verify/*blog*.mjs` (+ `.test.mjs` siblings), 11 `.github/workflows/blog-*.yml`
  workflows, and the npm chains `verify:blog:*` / `verify:blog:fba`.
- See Appendix A for the executed verifier results and §4 for the enforcement analysis.

---
## 3. Findings

Severity scale: **High** = correctness/data-integrity or security exposure on a production path;
**Medium** = contract, integration or operability defect with a realistic failure mode;
**Low** = hygiene, consistency, documentation or coverage defect.

### 3.1 High

#### H-1 — Public keyset pagination is inconsistent with the pinned-first ordering (missing rows / duplicates)

**Evidence**
- `src/services/post/queries.rs:461-463` — the public list orders by
  `is_pinned DESC, pinned_at DESC, published_at DESC, id DESC`.
- `src/services/post/queries.rs:445-455` — the `after` cursor filter uses **only** `published_at`/`id`
  (`PublishedAt.lt(after.published_at)` OR `PublishedAt = after AND Id < after.id`).
- `src/services/post/queries.rs:472` — `next_cursor` is built from the last returned row's `(published_at, id)`.

**Why it is wrong.** The ordering's leading key is the pin flag, and inside the pinned group the leading key is
`pinned_at`. The cursor cannot represent either. Concretely: with per_page = 2 and three pinned posts, page 1
returns pinned posts A and B (ordered by `pinned_at`). The cursor is `(B.published_at, B.id)`. Page 2 then
filters `published_at < B.published_at`, which excludes pinned post C whenever `C.published_at > B.published_at`
— so C is never returned, although it sorts above every non-pinned post. Symmetrically, rows can be returned
twice. Any storefront consumer that walks `nextCursor` therefore has a silent content-loss bug.

**Additional ordering hazard.** `pinned_at DESC` in PostgreSQL sorts NULLs first, and `create_post` can persist
`is_pinned = true` with `pinned_at = NULL` (see H-6), so a "created pinned" post floats to the very top of the
pinned block independently of `pinned_at`.

**Impact.** High — public content disappears from listing pages; unfixable at the client.
**Fix.** Encode the ordering key in the cursor (`is_pinned`, `pinned_at`, `published_at`, `id`) and compare
tuple-wise, or drop pinning from the keyset ordering and serve pinned posts as a separate, bounded
"featured" block fetched with its own query.

#### H-2 — Scheduled publishing is implemented as an unreachable method; no scheduler exists

**Evidence**
- `src/services/post/commands.rs:819` — `pub async fn publish_scheduled_posts(&self) -> BlogResult<Vec<(Uuid, Uuid)>>`.
- Repository-wide search for `publish_scheduled_posts` returns **only** that definition and one doc file
  (`docs/2026-10-09-blog-scheduled-publishing.md:44`, `:68`) which states the method is "ready to be called by
  an external scheduler worker".
- No binary, cron entry, task, or background worker exists: `apps/{server,admin,storefront,dioxus-admin}/src/main.rs`
  contain no scheduling wiring; `grep -rn "scheduler" apps/server/src/main.rs` is empty.
- `BlogPostScheduled` / `BlogPostScheduleCancelled` are published by `create_post`/`update_post`
  (`src/services/post/commands.rs:174-190`, `:520-550`) but no handler consumes them
  (`crates/modules/rustok-search/src/ingestion.rs:195-200` covers Created/Published/Unpublished/Updated/
  Archived/Deleted only).

**Additional defects in the method itself** (should be fixed when it is wired):
- It loads **every** tenant's due posts in one unbounded `.all(&self.db)` query with no tenant argument,
  no `LIMIT`, and no pagination.
- It opens one transaction per post; the first version-CAS conflict aborts the whole batch
  (`apply_status_transition_in_tx` returns `conflict`, propagating `?` out of the loop), leaving the rest
  unpublished until the next tick.
- It has no authorization surface (no `SecurityContext` argument, no tenant scoping) — a host that calls it from
  a request path would expose a cross-tenant publish primitive.
- The doc comment says "This method is called by the scheduler worker", which is false in this revision.

**Impact.** High — the feature is documented as delivered (H-1 in
`docs/audits/blog-deep-engineering-audit.md`) but cannot run; scheduled posts stay Draft forever unless an
operator calls the service manually.
**Fix.** Either ship the worker (a bounded, keyset-paginated, tenant-parameterized command that resumes after
conflicts) or mark the slice as source-only pending in `README.md` "Known limitations" and in the plan cursor;
add a tenant/actor parameter and a batch bound either way.

#### H-3 — Preview tokens: unauthenticated REST endpoint, unclamped TTL, no lifecycle cleanup

**Evidence**
- `src/controllers/posts.rs:410-448` — `GET /api/blog/preview/{token}` is **unauthenticated** (no `AuthContext`
  extractor), does **not** call `ensure_blog_module_enabled`, and defaults the locale to the hard-coded literal
  `"en"` (`let locale = params.locale.unwrap_or_else(|| "en".to_string());`) instead of
  `PLATFORM_FALLBACK_LOCALE` or the tenant default locale used everywhere else.
- `src/services/preview_token.rs:62` — `let ttl = ttl_hours.unwrap_or(DEFAULT_TTL_HOURS).min(MAX_TTL_HOURS);`
  the bound is one-sided: a negative `ttl_hours` produces an already-expired token that is still persisted, and
  `CreatePreviewTokenInput.ttl_hours` (`src/graphql/types.rs`) has no validator.
- `src/services/preview_token.rs:45-92` — `create_token` is documented as "for a draft post" but never checks the
  post status, so tokens are minted for published/archived posts as well.
- `src/services/preview_token.rs:94-127` — `get_post_by_token` deletes nothing, does not record usage, and loads
  the post through `SecurityContext::system()`, bypassing tenant/module gates (a token for a post in a disabled
  module still serves the body).
- `src/services/preview_token.rs:209` — `cleanup_expired()` has **no caller** anywhere in the repository.
- `src/services/post/commands.rs:748-806` — `delete_post` deletes tags, the post row, and routes, but **not**
  `blog_preview_tokens`, and `m20261009_000034_create_blog_preview_tokens.rs` creates **no foreign key** to
  `blog_posts` (see M-1), so deleted posts leave orphaned token rows behind forever.

**Impact.** High — unbounded orphan growth on a table with three indexes; leaked-content risk if a token outlives
its intended window; an unauthenticated endpoint that bypasses the module gate; inconsistent locale behaviour.
**Fix.** Clamp `ttl_hours` to `1..=MAX_TTL_HOURS`; require `PostStatus::Draft` (or explicitly allow and document);
delete tokens inside `delete_post`'s transaction; add the FK with `ON DELETE CASCADE`; route `cleanup_expired`
through the same worker story as H-2; make the REST handler use the canonical fallback locale and respect the
module gate (a preview token is not a tenant context).

#### H-4 — GraphQL rate limiting is fail-open for most of the Blog surface

**Evidence**
- `src/graphql/rate_limit.rs:49-57` — `BlogGraphqlSurface` contains exactly 10 variants:
  `post`, `postBySlug`, `posts`, `createPost`, `updatePost`, `deletePost`, `publishPost`, `unpublishPost`,
  `archivePost`, `moderateComment`.
- `src/graphql/rate_limit.rs:163-173` — only classified fields are inserted into
  `BlogGraphqlDocumentPolicy`; `src/graphql/rate_limit.rs:268-272` — if the policy is absent, the request is
  executed with **no limit check at all**.
- Missing from the classifier (all confirmed present as root fields in `src/graphql/{query,mutation}.rs`):
  `publicBlogPosts`, `postByPreviewToken`, `blogCategory`, `blogCategories`, `blogTags`, `createBlogComment`,
  `createBlogCategory`, `updateBlogCategory`, `deleteBlogCategory`, `createPreviewToken`,
  `revokePreviewToken`, `restorePost`, `bulkPublishPosts`, `bulkUnpublishPosts`, `bulkArchivePosts`,
  `bulkDeletePosts`, `pinPost`, `unpinPost`.
- No other limiter exists for this surface: `grep -rn "rate_limit" crates/modules/rustok-comments/src crates/libs/rustok-comments-api/src`
  returns nothing, so the public comment write path is completely unlimited.
- The verifier `verify-blog-graphql-rate-limit.mjs` passes because it asserts the presence of the **existing**
  list only; it contains no assertion that every Blog root field is classified.

**Impact.** High (abuse/cost). The spam-prone public write (`createBlogComment`), the most expensive public read
(`publicBlogPosts`), and every newly added write path are unmetered at the Blog GraphQL boundary.
**Fix.** Drive the surface table from the schema (a test that enumerates Blog root fields and asserts each one is
classified), and add explicit limits for comment creation, public lists, bulk operations and preview tokens.

#### H-5 — Bulk operations: unbounded input, raw internal errors returned to callers, N sequential transactions

**Evidence**
- `src/graphql/types.rs:784-787` — `BulkOperationInput { pub post_ids: Vec<Uuid> }` has no
  `#[graphql(validator(max_items = …))]`, and no service-level cap exists in
  `src/services/post/bulk.rs:40-52`.
- `src/services/post/bulk.rs:24` and `:46` — `result.record_failure(post_id, e.to_string())`.
- `src/dto/post.rs:451` — `record_failure(&mut self, post_id: Uuid, reason: impl Into<String>)`, surfaced to
  clients through `GqlBulkOperationFailure.reason` (raw `String`), which means
  `BlogError` display text — including `invariant` messages that carry internal identifiers such as
  "Blog tag usage projection underflow for Taxonomy term {id}" and "Blog category {id} has no canonical Taxonomy
  hierarchy placement" — is returned verbatim to API consumers. This contradicts the module's own
  single redacted public-error boundary (`src/error/public.rs`, asserted by `verify-module-reference-contract.mjs`).
- Each id is processed with its own transaction and its own event emission, so an N-id request is N transactions
  and up to N lifecycle events; combined with H-4 there is no rate limit on the same call.

**Impact.** High — internal-state disclosure to clients plus a cheap amplification vector (one request ⇒ N
transactions) on the write path.
**Fix.** Cap `post_ids` (e.g. `max_items = 100`), map failures through `BlogPublicError` and return a stable code
instead of display text, and either batch the work in one transaction per chunk or mark the operations as
resumable jobs.

#### H-6 — Pinning writes outside a transaction, without CAS, row-count verification or events; `create_post` can persist an inconsistent pin state

**Evidence**
- `src/services/post/pin.rs:47-72` — `set_pinned` performs a bare
  `blog_post::Entity::update_many().filter(Id).filter(TenantId).exec(&self.db)` with **no** `version` filter,
  **no** `rows_affected` check, and no transaction. It also writes `updated_at` **without** bumping `version`.
- No domain event is published on pin/unpin, so no search reindex/SEO notification is produced for a change that
  reorders public listings.
- `src/services/post/commands.rs:111-112` — `create_post` accepts `is_pinned` from the DTO
  (`is_pinned: Set(is_pinned.unwrap_or(false))`) and always stores `pinned_at: Set(None)`, regardless of the
  requested web `publish` flag or the resulting status. So `POST createPost(input:{isPinned:true, publish:false})`
  creates a Draft flagged as pinned with a NULL `pinned_at`, whereas `pin_post` rejects exactly that state
  ("Only published posts can be pinned", `pin.rs:44`).
- `m20261009_000035_add_blog_post_pinned.rs:38-47` creates `idx_blog_posts_pinned (tenant_id, is_pinned, pinned_at)`,
  but the public list also filters on `status` and `published_at IS NOT NULL` and orders by `published_at, id`,
  so the index cannot serve the public ordering as written.

**Impact.** High (state integrity + silent no-ops). A pin request racing a delete silently reports success; a
created-pinned Draft contradicts the pin contract; the search index is never refreshed for pinned ordering.
**Fix.** Route creation-time pinning through the same validation as `pin_post` (or drop `is_pinned` from
`CreatePostInput`), set `pinned_at` whenever `is_pinned` is set, perform pin/unpin in a transaction with a
`version` CAS or an explicit "metadata-only" documented decision, check `rows_affected`, and publish an event
(or document that pinning is intentionally index-neutral).

#### H-7 — Blog never consumes `TargetDeleted`; stale redirects surface as internal errors on public reads

**Evidence**
- `src/module.rs:96-101` registers exactly one handler (`BlogCommentProjectionHandler`); repository-wide search
  finds `TargetDeleted` consumers only in `rustok-comments` and `rustok-reactions`
  (`target_lifecycle.rs`), not in Blog.
- `DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md` requires that when a target is deleted, the
  owner table holding the redirect removes rows pointing at it, driven by a domain event, idempotently and
  retried until it succeeds.
- `src/services/post/repository.rs:249-256` — resolving a redirect whose target post no longer exists raises
  `BlogError::invariant("Blog redirect '{slug}' references missing post {id}")`. On the public
  `postBySlug`/storefront path this becomes an internal error (500-style, redacted) instead of "not found", and
  the stale row is never removed.
- `src/services/post/routes.rs:285-310` publishes `UrlAliasPurged`, but no consumer exists for it in this
  revision (the SEO handler at `rustok-seo/src/services/canonical_redirect_handler.rs:34` matches only
  `CanonicalUrlChanged`).

**Impact.** High for public reliability: a redirect whose target was deleted in another module (or by a
different owner path) turns a user-facing route into an internal error and an error-metric spike, and the row
persists indefinitely.
**Fix.** Register a Blog listener for `TargetDeleted` that deletes matching `blog_post_routes` rows idempotently
(and consumes `UrlAliasPurged` where relevant), and downgrade a dangling redirect to "not found" plus a metric
and a reconciliation task rather than an invariant failure.

### 3.2 Medium

#### M-1 — Entity/migration parity: a declared cascade that does not exist, and a cascade that is declared nowhere

- `src/entities/blog_preview_token.rs:19-27` declares `belongs_to blog_post` with `on_delete = "Cascade"`, but
  `m20261009_000034_create_blog_preview_tokens.rs` creates **no foreign key** — SeaORM consumers that rely on the
  relation would be wrong, and the DB keeps rows for deleted posts.
- `src/entities/blog_post_tag.rs:19-23` declares the same relation **without** a cascade, while
  `m20260813_000018_enforce_blog_post_tag_tenant_integrity.rs` replaces the FK with a composite
  `(tenant_id, post_id) … ON DELETE CASCADE`.
- `src/entities/blog_post_route.rs` declares no relations and `m20261008_000032` creates no FK, so dangling
  redirects are only prevented by code (interacts with H-7).
- **Fix**: make migrations and entities agree in both directions; add a parity check to the verifier set.

#### M-2 — Published posts can never be hard-deleted, so `TargetDeleted`/route purge never fires for them

`src/services/post/commands.rs:748-760` returns `CannotDeletePublished` for published posts, and there is no
trash state, no unpublish-then-delete combined command, and no admin UI flow for it. Consequently the only
content that ever reaches the `BlogPostDeleted`/`TargetDeleted` path is Draft/Archived content, and every
redirect/SEO/reaction cleanup bound to deletion is effectively limited to that subset. **Fix**: define a
supported deletion path for published content (archive → delete, or delete-with-unpublish), and document it.

#### M-3 — `list_tags` still uses OFFSET pagination with a COUNT, and its total can disagree with its page

`src/services/tag.rs:181-215` uses `Entity::find().paginate(&txn, per_page)` plus `num_items()` and page
arithmetic, and returns `total` derived from `blog_tag_usage` rows. The ADR bans `OFFSET` on hot paths and
requires keyset pagination; `verify-blog-tag-pagination-source.mjs` explicitly expects
`let offset = tag_page_offset(page, per_page);` in `tag.rs` and fails today (12 missing markers). The same
verifier requires evidence/plan status text that does not exist, so the slice is genuinely not implemented
(not merely a broken script). Since a visible tag additionally requires a Taxonomy term to exist
(`tag.rs:218-232` returns `invariant` otherwise), `total` can exceed the number of returned items.

#### M-4 — `blog_tag_usage` has no rebuild/backfill path, and drift turns author operations into invariant failures

`decrement_tag_usage_in_tx` (`src/services/tag.rs:606-641`) requires `rows_affected == 1` for a guarded
`use_count - 1` update and otherwise raises `invariant` ("Blog tag usage projection underflow"). Therefore any
projection drift (a tag attached outside `TagService`, a partially applied historical migration, manual DB
repair) makes **post update and post delete fail permanently** for the affected posts, with an internal error and
no repair path in the module. `list_tags` similarly raises `invariant` if a usage row references a missing or
non-module term. **Fix**: ship an idempotent rebuild command (recompute from `blog_post_tags` × Taxonomy scope)
and, until then, treat missing/zero rows as "recompute + warn" rather than an invariant failure.

#### M-5 — Public-facing Category and Tag listings remain OFFSET + totals

`src/graphql/query.rs:324-395` (`blogCategories`, `blogTags`) exposes `page`/`per_page` and returns `total`
from `CategoryService::list` / `TagService::list_tags`, both of which count rows and use offset pagination. These
are public read paths, i.e. exactly what the ADR's "keyset everywhere on hot paths, OFFSET banned there" rule
targets. The admin `posts` list, by contrast, is keyset-based (`nextCursor` only, "Admin lists never count
rows" in `src/graphql/types.rs:329-332`) — the two read styles now coexist with different guarantees.

#### M-6 — The published OpenAPI document omits three live REST endpoints

`src/controllers/openapi.rs` lists 16 handlers; `preview_post`, `pin_post` and `unpin_post` are absent even
though their handlers carry full `#[utoipa::path]` attributes (`src/controllers/posts.rs:411`, `:454`, `:492`).
Any generated client/spec consumer will not see the preview, pin or unpin endpoints. **Fix**: add them, and add a
verifier that every `#[utoipa::path]` handler appears in `BlogApiDoc`.

#### M-7 — New backend capabilities have no UI in either stack

- Leptos admin: no pin/unpin, bulk, scheduling, or preview-token controls; `is_pinned` is always sent as `None`
  (`admin/src/transport/graphql_adapter.rs:224,258`; `admin/src/transport/native_server_adapter.rs:304,367`).
- Next.js admin (`apps/next-admin/packages/blog`): no pin, bulk, or preview-token support; the post form does
  carry `scheduledAt`.
- Storefronts: no pinned/featured affordance; pinned ordering is only implicit via the backend sort (which is
  itself broken per H-1).

**Impact**: pin/unpin, bulk operations and preview tokens are API-only features — the "feature closed" claims for
M-5/H-4/H-6 of the historical audit are not user-visible.

#### M-8 — Documentation drift against the source

- `README.md:177-192` "Known limitations" still lists "No scheduled publishing (H-6)", "No trash, draft preview,
  or revision history (M-3, M-4)", and "The Leptos storefront does not redirect retired slugs (M-12)", while
  slices 100-106 added scheduled publishing, preview tokens, pinning and bulk operations (with the caveats in
  H-2/H-3/H-6). The audit identifier mapping is also inconsistent with
  `docs/audits/blog-deep-engineering-audit.md` (there, scheduled publishing is H-1 and bulk operations H-6).
  Per `AGENTS.md` §14, unimplemented/partial production behaviour must be documented in the owning component's
  README under the known-limitations section — it currently overclaims in one direction (no scheduling, no
  preview) and under-documents in the other (pinning/bulk are not mentioned at all).
- The predecessor audit `docs/audits/blog-deep-engineering-audit.md` states that Blog owns routes through the
  `rustok-content` canonical URL registry (`CanonicalUrlWriter`), which the ADR of 2026-10-08 retired and the
  source already replaced with `BlogPostRouteOwner` + `blog_post_routes`.
- `CRATE_API.md:9-11` asserts "Persistence entities are **not** public API … `entities` is crate-private.
  Direct cross-crate SeaORM access is not supported", while `src/lib.rs:44` re-exports
  `ports::{blog_category, blog_post, blog_post_tag, blog_post_translation}` publicly
  (documented as "deliberate re-exports" in the crate docs). The claim and the code cannot both be true.
- `CRATE_API.md:61` contains a literal `\n\n` escape sequence inside prose (formatting defect).
- The plan status `canonical_reference_v1_source_certified_execution_evidence_pending`
  (`docs/implementation-plan-current.md:3`) coexists with mutually contradictory verifier requirements (M-9).

#### M-9 — The Blog verifier set is internally contradictory and cannot all pass

- `verify-blog-canonical-plan-current.mjs` (exit 28) requires
  `contracts/evidence/blog-category-translation-postgres-source.json` to exist and requires status markers such
  as `tag_list_pagination = source_ready_maintainer_execution_pending` for features that are **not implemented**
  (see M-3).
- `verify-blog-taxonomy-category-planning-cursor.mjs` requires that same evidence file to be **absent**, and
  requires `src/entities/blog_category_translation.rs` to exist, while
  `verify-module-reference-contract.mjs` explicitly fails if that file exists ("retired Blog category
  translation entity must not exist").
- `verify-blog-tag-mutation-reindex-source.mjs` requires `enforce_scope(…, Resource::Taxonomy, …)` calls inside
  `crates/modules/rustok-taxonomy/src/module_term_mutation.rs` and plan markers that do not exist, while
  `verify-blog-tag-canonical-projection-source.mjs` requires `resolve_term_names(...)` in `tag.rs` and
  `blog_post_tag::Entity::delete_many()` in `tests/taxonomy_tags.rs`, none of which exist.

**Impact**: "the machine gates are `npm run verify:module-source-layout` and
`npm run verify:module-reference-contract`" (plan) is true only for those two gates; the wider Blog gate set
contradicts itself and cannot be green, so it cannot certify anything.

#### M-10 — Most Blog verifiers cannot run in CI, and a fifth of them cannot run at all

- A reachability scan (workflow files + `package.json` script closure) finds **8 of 67** Blog verifiers reachable
  from any workflow; the remaining **59 are orphaned**, including `verify-blog-fba.mjs`, the whole
  `verify:blog:*` chain, `verify-blog-canonical-plan-current.mjs`, the admin/storefront boundary verifiers, the
  GraphQL rate-limit and richtext boundary verifiers, and every `verify-blog-comments-*` verifier.
- No workflow runs `npm run verify:blog*` (`grep -rn "verify:blog" .github/workflows/` → no matches). The 11
  `blog-*.yml` workflows cover only Taxonomy/Category contracts (mostly `cargo test`) plus a handful of node
  scripts.
- At least 20 verifiers crash with a JavaScript `SyntaxError` before any assertion, because a string contains an
  unescaped `"` inside a double-quoted literal — e.g. `verify-blog-category-hierarchy-command.mjs:88` embeds
  `format!("blog-category-tree:{tenant_id}")`, and the `verify-blog-comments-audit-*` / `verify-blog-comments-tcp-*`
  families embed Rust code the same way. Those verifiers are dead code that always "fail loud" if ever run.
- `verify-blog-comments-host-provider-selection.mjs` fails solely on whitespace: it requires the literal
  `extensions.insert::<Arc<dyn CommentsThreadPort>>` on one line, but
  `apps/server/src/services/comments_provider_runtime_base.rs:255-256` contains
  `extensions\n    .insert::<Arc<dyn CommentsThreadPort>>(` after rustfmt. The capability is present.
- `verify-blog-reaction-subject-provider.mjs` fails because it requires the provider to call
  `blog_post::Entity::find()` directly; the implementation checks existence through
  `load_post_subject_snapshot` (`src/integrations/reaction_subject.rs:96-101`), which is equivalent but
  differently spelled.

**Impact**: the strongest available "verification" mechanism for Blog is partially unexecutable, partially
unwired and partially stale — which is how a stale README and a contradictory plan could survive.

#### M-11 — `revoke_token` reports a missing token as a missing post

`src/services/preview_token.rs:134-148`: when the token id is not found (or belongs to another tenant), the
service returns `BlogError::PostNotFound(token_id)`. The API therefore answers a missing/foreign token with a
"post not found" code and message, which is misleading for clients and hides the difference between "token
already revoked" and "post gone". **Fix**: add a dedicated `PreviewTokenNotFound` case (or return `Ok(false)` for
idempotent revoke).

#### M-12 — Admin GraphQL listing has no sort control

`PostsFilter` (`src/graphql/types.rs`) exposes filters and `after`/`per_page` but no sort fields, and
`src/graphql/query.rs:138-175` hard-codes `sort_by: CreatedAt, sort_order: Desc` before calling
`list_posts_with_locale_fallback`, even though the service and DTO support `PostSortField`/`PostSortOrder`
(and the migration `m20260328_000001` added one keyset index per sort column). The historical audit's M-15
("limited sorting in admin") therefore remains open, and the per-sort-column indexes are currently unused by the
GraphQL read path.

#### M-13 — Preview tokens ignore the module gate and post status by design of omission

Beyond H-3's REST issues, `get_post_by_token` (`src/services/preview_token.rs:94-127`) never calls
`require_module_enabled`/`ensure_blog_module_enabled` and never re-checks that the tenant module is enabled, so a
token minted before a tenant disabled Blog keeps serving content afterwards. The same function loads the
translation through `SecurityContext::system()`, so no per-request authorization is applied at all: the token is
the entire authorization model, with no revocation-on-status-change (e.g. archiving a post does not invalidate
its tokens) and no audit of preview reads. Documented behaviour should at minimum state this explicitly
(`README.md` currently says nothing about tokens besides the stale "no draft preview" line).

### 3.3 Low

- **L-1** `create_post`/`update_post` begin their transaction before input validation (e.g.
  `src/services/post/commands.rs:86-90` returns a validation error after `self.db.begin()`), holding a pooled
  connection and rolling back implicitly on drop. Validate before opening the transaction.
- **L-2** `UrlAliasPurged` is published (`src/services/post/routes.rs:302`) but has no consumer anywhere in the
  repository, so its ADR-mandated purge/retry semantics are unimplemented (see H-7).
- **L-3** `apps/next-admin/packages/blog/src/api/posts.ts` selects `scheduledAt` twice in `POSTS_QUERY`
  (duplicate selection, harmless per spec, but a defect in the document).
- **L-4** `src/lib.rs:44` publicly re-exports four persistence entities, contradicting `CRATE_API.md`'s privacy
  claim (see M-8). Either remove the re-exports or document them as a supported (deprecated) surface.
- **L-5** `src/migrations/m20260803_000011_create_blog_comments_audit_recovery.rs:14` uses
  `unreachable!("unsupported SeaORM database backend")`, i.e. a panic on a runtime code path; sibling migrations
  return `DbErr::Custom` for unsupported backends. Make it consistent.
- **L-6** `AGENTS.md` §14 forbids `unwrap`/`expect`/`panic!` in non-test Rust runtime code, yet runtime code
  contains `.expect("builtin SEO target slug must stay valid")` (`src/integrations/seo_targets.rs:29,169,185,223`),
  `.expect("Blog reaction source constant must remain valid")` and
  `.expect("Blog post reaction kind constant must remain valid")` (`src/integrations/reaction_subject.rs:181,187`)
  and `unreachable!("disabled comments policy returned before provider read")`
  (`src/integrations/public_comments_snapshot.rs:164`). These are invariant-by-construction constants, but they
  are still panics on runtime paths and should be `LazyLock`/`static` conversions or explicit error mapping.
- **L-7** No owner tests exist for the newest slices: a search of `tests/` and `src/services/post/tests.rs` finds
  only `is_pinned` fixture fields and no test exercising `PreviewTokenService`, `pin_post`/`unpin_post`,
  `bulk_*`, or `publish_scheduled_posts`. (Compilation/tests could not be run here; this is a coverage statement
  about source, not about pass/fail.)
- **L-8** `docs/audits/blog-deep-engineering-audit.md` is written entirely in Russian, violating `AGENTS.md` §12
  ("repository artifacts in English only"; the sole exception is `README.ru.md`). It should be translated,
  retitled as historical, or retired.
- **L-9** The continuous-review gate has no Blog coverage: `.review_ledger.json` shows `current_round: 1`,
  `completed_rounds: []` and every component `completed: false`, and `.review_backlog.json` (2026-10-07) contains
  no Blog path at all. The module's "certified reference" status therefore rests solely on the (partially dead)
  source verifier set.

---
## 4. Verification and governance assessment

### 4.1 What the module claims vs. what can be demonstrated

| Claim (source) | Verifiable in this environment | Assessment |
|---|---|---|
| "The machine gates are `npm run verify:module-source-layout` and `npm run verify:module-reference-contract`" (`docs/implementation-plan-current.md:52`) | Both run and pass in this environment | **Holds** (as string-level gates only; no compile evidence) |
| "Status: `canonical_reference_v1_source_certified_execution_evidence_pending`" | The plan-cursor verifier that is supposed to enforce the cursor text fails with 28 missing markers | **Contradicted by its own gate** (M-9) |
| "Comments FBA remains `boundary_ready`" | Consistent with the source (`rustok-module.toml` FBA block, port-based integration) | Holds as a boundary statement |
| Scheduled publishing / preview / pinning / bulk delivered (slice docs `docs/2026-10-09-blog-*.md`) | Source exists but is unwired (H-2), unsafe at the edges (H-3, H-6) and untested (L-7) | **Partially delivered; README still says the opposite** (M-8) |
| `README.md` "Known limitations" | Stale relative to slices 100-106 and to the retired canonical-URL registry | **Not accurate** |
| Continuous review coverage (`AGENTS.md` §2.1, `.review_ledger.json`) | Round 1, zero completed rounds, no Blog path in the backlog | **Not covered** |

### 4.2 Classification of failing verifiers

| Verifier | Result | Classification | Reasoning |
|---|---|---|---|
| `verify-blog-canonical-plan-current.mjs` | FAIL (28 markers) | Contradictory expectation (M-9) | Demands markers/evidence that another verifier forbids |
| `verify-blog-taxonomy-category-planning-cursor.mjs` | FAIL | Contradictory expectation (M-9) | Requires the retired entity that `verify-module-reference-contract.mjs` forbids |
| `verify-blog-taxonomy-category-{dual-write,structure-sync,read-cutover}.mjs` | FAIL | Stale expectation | Expects legacy binding/donor markers the Taxonomy cutover intentionally retired |
| `verify-blog-post-category-name-projection-source.mjs` | FAIL (9) | Stale expectation | Expects `blog_category_taxonomy_binding`-based resolution; the read path now uses the canonical Taxonomy reader |
| `verify-blog-tag-canonical-projection-source.mjs` | FAIL (4) | Mixed: stale markers + real gaps | `resolve_term_names`/test markers are planning-era; the underlying canonical projection path is nevertheless worth re-specifying |
| `verify-blog-tag-pagination-source.mjs` | FAIL (12) | **Real gap** (M-3) | Tag list genuinely still uses OFFSET + COUNT and the plan markers are absent |
| `verify-blog-tag-mutation-reindex-source.mjs` | FAIL (6) | Cross-owner gap | Requires `enforce_scope(…, Resource::Taxonomy, …)` in `rustok-taxonomy`; the Taxonomy module-term mutation entry points do not perform their own RBAC check (Blog enforces upstream) — worth an owner decision |
| `verify-blog-comments-audit-outbox-relay-postgres-source.mjs` | FAIL (2) | Docs/evidence drift | Requires slice-96 cursor text and a "Delivery remains owned by" phrasing not present |
| `verify-blog-comments-host-provider-selection.mjs` | FAIL (1) | **Script defect** | Whitespace-fragile literal (`extensions\n.insert::<…>`); the capability exists (M-10) |
| `verify-blog-reaction-subject-provider.mjs` | FAIL | **Script defect (stale literal)** | Requires a direct entity query; the equivalent snapshot loader is used |
| `verify-blog-category-hierarchy-command.mjs` and the `verify-blog-comments-{audit,tcp}-*.mjs` families (~20 scripts) | `SyntaxError` | **Script defect** | Unescaped `"` inside double-quoted JS string literals (M-10) |
| `verify-blog-taxonomy-category-{delete-lifecycle,mutation-response,mirror-write-retirement,mutation-cutover}.mjs` | Not present on disk | Naming mismatch | The four same-named CI workflows (`blog-taxonomy-category-{delete-lifecycle,mutation-response,legacy-storage-retirement,mirror-write-retirement}.yml`) exist, but they run `cargo test` contracts and/or differently named node scripts (`verify-blog-taxonomy-category-dual-write.mjs`, `verify-blog-taxonomy-category-structure-sync.mjs`, `verify-blog-category-hierarchy-command.mjs`), so the missing node scripts have no CI owner |

### 4.3 CI wiring

- No workflow invokes `npm run verify:blog*`; the `verify:blog:fba` chain (which itself embeds scripts that are
  red today) is therefore never executed by CI.
- 59/67 Blog verifiers are unreachable from any workflow; the 8 that are reachable belong to the
  Taxonomy/Category contract family.
- `hardening-gates.yml` runs the two module-wide gates (`verify-module-source-layout`,
  `verify-module-reference-contract`), which pass.
- At the audited revision, `main` CI runs were red overall (`CI`, `Browser E2E`, `Rust-hosted Browser Smoke`,
  `Dependency Advisory Reachability`, `Fly Page Builder` failed; `Hardening Gates`, `Ecommerce Hardening`,
  `Repository Ruleset Contract`, both CodeQL runs passed). This audit read the run metadata via `gh`; it did not
  reproduce those jobs, and none of the failures are attributed to Blog by this audit.

### 4.4 Documentation-governance findings

- Stale owner README (M-8) — the single most consequential governance defect, because `AGENTS.md` §14 explicitly
  makes the README the place where production gaps must be declared.
- Russian-language audit artifact under `docs/audits/` (L-8) — violates the English-only artifact rule.
- Plan-cursor verifier vs. actual plan text (M-9).
- Two "canonical" descriptions of the same Blog URL mechanism (owner routes vs. canonical URL registry) still
  coexist in the repository; only the former exists in source.

---

## 5. Explicitly unverified areas (no evidence produced by this audit)

All of the following must be treated as **unverified**, regardless of what any contract file asserts:

1. **Compilation** of `rustok-blog`, `rustok-blog-admin`, `rustok-blog-storefront`, `apps/server`, `apps/admin`,
   `apps/storefront`, `apps/next-admin`, `apps/next-frontend`. (`cargo`/`rustc` unavailable.)
2. **Rust unit/property/integration tests**, including all comment-projection PostgreSQL suites, the Category
   taxonomy cutover suites, the rate-limit policy tests and `sqlite`/`postgres` migration execution.
3. **SQL correctness of the 29 migrations** on a live PostgreSQL/SQLite instance (triggers, composite FKs,
   append-only trigger behaviour, the `000029` settings backstop, the `000011` recovery trigger).
4. **GraphQL schema execution**: no SDL snapshot was generated; the `verify-blog-graphql-*` scripts are string
   matchers, not schema validation.
5. **Runtime wiring in `apps/server`** (provider selection, TCP comments path, keyring/delegation schedule
   handling) beyond the static call-chain reading in §2.8.
6. **Frontend rendering** (Leptos SSR/hydration, Next.js admin/storefront pages, RSS feed output, JSON-LD).
7. **CI behaviour at HEAD for the blog workflows** — the workflow definitions and run metadata were inspected,
   but the taxonomy contract cargo jobs were not reproduced.
8. **Performance claims** (keyset vs. OFFSET, N+1 risks such as the per-post `public_comments`/
   `moderation_comments` fields noted in the historical audit) — no query plans, no benchmarks.

Any statement implying that these were verified would violate `AGENTS.md` §15.

---

## 6. Recommended remediation order

**Before any further feature work**

1. Fix the public listing pagination contract (H-1) — smallest change with the largest user-visible correctness
   win. Add a regression test that pins three posts with out-of-order `published_at` values and walks the cursor.
2. Decide the scheduler story (H-2): ship the bounded worker or downgrade the docs to "source-only, no
   executor".
3. Close the preview-token holes (H-3, M-11, M-13): TTL clamp, draft-only or documented status policy, deletion
   inside `delete_post`, FK + cascade, cleanup caller, REST handler module gate + canonical locale.
4. Extend the rate-limit surface table to every Blog root field and add a test that enumerates them from the
   schema (H-4).
5. Bound bulk input and redact bulk failure reasons (H-5).
6. Make pinning transactional and consistent between `create_post` and `pin_post`, and emit (or explicitly
   waive) an event (H-6).
7. Add the `TargetDeleted` consumer for `blog_post_routes` and downgrade dangling-redirect resolution to
   not-found + metric + reconciliation (H-7).

**Governance / enforcement (parallel track)**

8. Repair the verifier set: escape or template the ~20 `SyntaxError` scripts, make the provider-selection matcher
   whitespace-insensitive, replace stale literals with structural assertions, and resolve the contradictory
   plan/entity expectations (M-9, M-10).
9. Wire the Blog verifier chain into a workflow (including a `verify:blog:*` job) so that red scripts are visible
   (M-10, §4.3).
10. Rewrite `README.md` "Known limitations" against the actual source, translate/retire the Russian audit file,
    and correct `CRATE_API.md` (M-8, L-4, L-8).
11. Add a schema/manifest parity test for the OpenAPI document and for entity↔migration FKs (M-1, M-6).
12. Provide a tag-usage rebuild command and make drift recoverable rather than invariant-fatal (M-4).

---

## Appendix A — Node verifier execution results (this audit)

Environment: node v22.22.3, executed from the repository root; each script run with a 60 s timeout.
"PASS" means exit code 0 with no failure output.

**PASS (executed in this audit)**: `verify-module-source-layout`, `verify-module-reference-contract`,
`verify-blog-admin-boundary`, `verify-blog-storefront-boundary`, `verify-blog-ai-richtext-boundary`,
`verify-blog-category-search-reindex`, `verify-blog-comments-admin-native-port-injection`,
`verify-blog-comments-dispatcher-duplicate-delivery`, `verify-blog-comments-duplicate-delivery-race`,
`verify-blog-comments-event-projection`, `verify-blog-comments-graphql-port-injection`,
`verify-blog-comments-http-port-injection`, `verify-blog-comments-port-boundary`,
`verify-blog-comments-remote-adapter-core`, `verify-blog-comments-storefront-native-port-injection`,
`verify-blog-comments-storefront-write-surface`, `verify-blog-comments-tcp-server-adapter`, `verify-blog-fba`,
`verify-blog-fba-verification-chain`, `verify-blog-forum-ui-ownership`, `verify-blog-graphql-rate-limit`,
`verify-blog-graphql-richtext-boundary`, `verify-blog-taxonomy-category-central-registry`,
`verify-blog-taxonomy-category-cross-owner-docs`, `verify-blog-taxonomy-category-live-plan`,
`verify-blog-taxonomy-category-translation-docs`.

**FAIL** (line counts in parentheses where the script reports them):

| Script | Reported failure |
|---|---|
| `verify-blog-canonical-plan-current` | 28 missing markers (plan/README cursor text) |
| `verify-blog-tag-pagination-source` | 12 missing markers, incl. `let offset = tag_page_offset(page, per_page);` |
| `verify-blog-post-category-name-projection-source` | 9 missing markers (retired binding-table expectations) |
| `verify-blog-tag-mutation-reindex-source` | 6 missing markers (Taxonomy `enforce_scope` + plan status) |
| `verify-blog-tag-canonical-projection-source` | 4 missing markers |
| `verify-blog-taxonomy-category-dual-write` | missing crate-private donor translation entity |
| `verify-blog-taxonomy-category-structure-sync` | 7 missing placement/synchronization markers |
| `verify-blog-taxonomy-category-read-cutover` | 6 missing binding/delegation markers |
| `verify-blog-taxonomy-category-planning-cursor` | requires `src/entities/blog_category_translation.rs` |
| `verify-blog-comments-audit-outbox-relay-postgres-source` | 2 missing doc/cursor markers |
| `verify-blog-comments-host-provider-selection` | whitespace-fragile literal (script defect) |
| `verify-blog-reaction-subject-provider` | stale literal (script defect) |
| `verify-blog-category-hierarchy-command` + ~20 `verify-blog-comments-{audit,tcp}-*.mjs` | `SyntaxError` (script defect) |

**Absent from disk** (while same-named CI workflows exist):
`verify-blog-taxonomy-category-delete-lifecycle.mjs`, `...-mutation-response.mjs`,
`...-mirror-write-retirement.mjs`, `...-mutation-cutover.mjs`.

## Appendix B — Inspection coverage

Files read in full or in substantive part during this audit (module-relative unless stated):
`AGENTS.md`; `README.md`; `CRATE_API.md`; `REVISION_INTEGRATION_GUIDE.md`; `rustok-module.toml`; `docs/README.md`;
`docs/implementation-plan-current.md`; the five `docs/2026-10-09-blog-*.md` slice records;
`docs/audits/blog-deep-engineering-audit.md`; `DECISIONS/2026-10-08-owner-routes-and-batched-conversion.md` and
`2026-10-08-blog-post-url-and-publication-contract.md`; `docs/standards/rustok-blog-reference-audit-context-2026-09-21.md`;
`docs/standards/cross-cutting-review-ledger.md`; `.review_ledger.json`; `.review_backlog.json`; `package.json`;
`modules.toml`; `.github/workflows/blog-*.yml` (11 files) and `hardening-gates.yml`; `scripts/verify/*blog*.mjs`
(all ~67, executed where runnable); `build.rs`;
`src/{lib,module}.rs`; `src/domain/{state_machine,state_machine_proptest,richtext,comment_policy}.rs`;
`src/dto/{post,category,category_command,comment,tag,mod}.rs`; `src/entities/*.rs` (9);
`src/migrations/mod.rs` plus migrated-table/trigger inspections across `000001`–`000035`;
`src/services/{mod,rbac,tag,tag_tenant_integrity_tests,category,category_command,category_delete,category_owner,
category_name_projection,category_taxonomy_sync,taxonomy_translation_owner,comment,comment_projection,
preview_token}.rs`; `src/services/post/{mod,commands,queries,repository,routes,bulk,pin,tests}.rs`;
`src/controllers/{mod,posts,categories,comments,comment_assets,openapi}.rs`;
`src/graphql/{mod,query,mutation,types,rate_limit,runtime_data}.rs`;
`src/integrations/{seo_targets,reaction_subject,public_comments_snapshot,mod}.rs`; `src/ports/mod.rs`;
`src/error/{mod,public}.rs`; `admin/src/**` (structure + pin/preview/schedule plumbing); `storefront/src/**`
(structure + cursors); `apps/next-admin/packages/blog/**`; `apps/next-frontend/packages/rustok-blog/**` and the
`/[locale]/blog` routes; `apps/server/src/services/{mod,comments_provider_runtime*.rs}`;
`crates/modules/rustok-comments/src/{ports,public_read}.rs`; `crates/modules/rustok-search/src/{blog_projector,ingestion}.rs`;
`crates/modules/rustok-taxonomy/src/module_term_mutation.rs` (markers);
`crates/modules/rustok-content/src/migrations/` (enumeration); `crates/libs/rustok-events/src/types/*`.

Not read in full: the remaining `rustok-blog/tests/*.rs` suites (enumerated and asserted only by name/shape),
the `contracts/evidence/*.json` payloads (spot-checked via their verifiers), and the full bodies of the
comments delegation-audit migrations (`000007`–`000011`) beyond their DDL and trigger definitions.
