## 2026-06-07 — SEO Phase D (D5-D7) batch progress

- Hardened SEO index repair/replay idempotency: historical replay now deduplicates repeated operator runs and reuses transition keys across replay retries.
- Added replay failure-recovery coverage (`historical_replay_deduplicates_repeat_runs`, `historical_replay_retries_failed_delivery_without_duplicate_rows`) and transport parity coverage (`memory` + `streaming` reliability levels).
- Extended Next Admin SEO operator panel with failure drilldown (`failureSamples`) and updated GraphQL/REST contract mapping.
- Added renderer parity snapshot tests, including deterministic primary snapshot assertion and token-based normalization for non-deterministic payload fields.
- Added SEO replay/repair operational runbook with troubleshooting and verification checklist.

# Changelog

All notable changes to RusToK are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Newsletter Module (M-9)
- **New standalone module `rustok-newsletter`** for cross-domain email campaign management
- `rustok-newsletter-api` crate with contracts:
  - `NewsletterContentProvider` trait for source modules (blog, forum, commerce)
  - `SubscriberPort` and `CampaignPort` typed ports
  - Shared types: `ContentSourceSlug`, `SubscriberStatus`, `CampaignStatus`
- `rustok-newsletter` implementation crate (29 files):
  - Domain: subscriber/campaign lifecycle state machines with validated transitions
  - Services: `SubscriberService` (subscribe, confirm, unsubscribe, update, delete) and `CampaignService` (create, update, schedule, cancel, delete)
  - Entities: 3 tables (`newsletter_subscribers`, `newsletter_campaigns`, `newsletter_subscriptions`)
  - Migration `m20261009_000040_create_newsletter_tables` with 6 indexes
  - GraphQL: 4 queries + 10 mutations for subscriber and campaign management
  - Ports: typed port implementations following platform contracts
  - Module registration: `rustok-module.toml`, workspace integration
- Architecture: Newsletter aggregates content from blog/forum/commerce via `NewsletterContentProvider` trait (inversion of control pattern)

#### Content Portability Platform (M-7)
- **New platform capability** for unified content import/export across all modules
- `rustok-content-portability-api` crate with contracts:
  - `ContentImporter<Source, Target>` and `ContentExporter<Source, Target>` traits
  - Format descriptors: JSON, CSV, WordPress XML, Markdown, Custom
  - Context types: `ImportContext`, `ExportContext` with tenant isolation
  - Result types: `ImportResult<T>`, `BatchImportResult<T>`, `ExportResult<T>`
  - Validation framework: `ValidationResult`, `FieldValidation`, `validate_fields!` macro
  - Progress tracking: `ProgressCallback` for batch operations
- `rustok-content-portability` implementation crate:
  - `JsonFormatHandler`: JSON parse/serialize with pretty-print option
  - `CsvFormatHandler`: CSV parse/serialize with custom delimiters and headers
  - `ImportService`: import from JSON/CSV files and bytes
  - `ExportService`: export to JSON/CSV files and bytes
  - File I/O helpers: `read_file`, `write_file`, `file_exists`, `file_size`
  - Comprehensive unit tests for all components
- Architecture: Platform capability (support crate), not tenant-toggled module, following `rustok-api`/`rustok-core` pattern

#### Blog Module Enhancements (M-5)
- **Featured/Pinned Posts** feature:
  - Migration `m20261009_000035_add_blog_post_pinned`: `is_pinned` boolean + `pinned_at` timestamp with composite index
  - Entity fields: `is_pinned: bool`, `pinned_at: Option<DateTimeWithTimeZone>`
  - DTO: `is_pinned`+`pinned_at` in `PostResponse`/`PostSummary`; pin state is deliberately **not**
    an input field, so `CreatePostInput`/`UpdatePostInput` cannot bypass the published-only rule
  - Service: `pin_post`/`unpin_post` commands, implemented by a private transactional, row-locked
    `set_pinned` helper (only published posts can be pinned; a concurrently deleted post fails closed)
  - Queries: public listing sorts by `is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC, id DESC`,
    and `PublishedPostCursor` carries that whole sort key, so keyset pages neither skip nor repeat rows
    once pinned posts exist (legacy `(published_at, id)` tokens are rejected, not resumed lossily)
  - GraphQL: `isPinned: Boolean!`, `pinnedAt: String`, `pinPost(id)`, `unpinPost(id)` mutations
  - REST: `POST /api/blog/posts/{id}/pin` and `POST /api/blog/posts/{id}/unpin` endpoints
- Blog scheduled publishing is wired end to end: `PostService::publish_scheduled_posts` requires an
  explicit `tenant_id` and a batch `limit` (clamped to 200), publishes oldest-due first, and skips a
  concurrently edited post instead of aborting the batch. The Blog-owned
  `blog_scheduled_publish` module-work registration (`src/services/scheduler.rs`) publishes the durable queue to
  the host scheduler, which drains it in `full`/`worker` host mode; the module imports no local timer.

#### Blog hardening (2026-10-15 engineering audit)
- Preview tokens are bounded and draft-only: the TTL is validated against `1..=720` hours instead
  of being stored as an already-expired or 30-day-plus token, and a token can only be minted for an
  unpublished draft by the post author or a caller with post update permission.
  `get_post_by_token` re-checks that the token's own tenant still has Blog enabled before it serves
  the post, so a token minted before a tenant disabled the module stops working.
- `PreviewTokenService::revoke_token` reports a missing or foreign token as
  `PreviewTokenNotFound`/`PREVIEW_TOKEN_NOT_FOUND` instead of a missing post.
- Preview token retention is tenant-scoped: `cleanup_expired(tenant_id, limit)` (limit clamped to
  `MAX_PREVIEW_TOKEN_CLEANUP_BATCH` = 500) drains one tenant's expired tokens from the
  `blog_preview_token_cleanup` module-work registration (`src/services/scheduler.rs`), and deleting a post
  removes its tokens through the `fk_blog_preview_tokens_post` cascade.
- Blog GraphQL rate limiting classifies every schema root: all 28 Blog root fields (8 queries, 20
  mutations) map to a `BlogGraphqlSurface`, so a new root cannot silently leave the limiter's
  surface table, and a write whose actor lacks the field's permission is not counted (RBAC stays
  authoritative). `blog_schema_root_fields_are_fully_classified` fails when a root is unclassified.
- Bulk post operations are bounded per request (`MAX_BULK_POST_IDS` = 100, mirrored by
  `#[graphql(validator(max_items = 100))]`), and `BulkOperationResult::record_failure` reports each
  failure through the redacted `BlogPublicError` boundary, so internal `Invariant`/`Database`
  diagnostics no longer reach API consumers.
- Blog consumes `TargetDeleted` for its own posts: `blog_target_deletion_routes`
  (`src/integrations/target_lifecycle.rs`) removes the routes of a post deleted outside the Blog
  delete path and reports them through `UrlAliasPurged`, idempotently on repeated delivery. A read
  of a route whose target is gone now returns "not found" instead of resolving a stale redirect,
  records the `dangling_route_redirect` module metric, and leaves the row to
  `blog_route_reconciliation`, which sweeps stale routes in bounded, tenant-scoped batches.

- The authenticated Blog GraphQL post list exposes `sortBy`/`sortOrder`
  (`BlogPostSortField`/`BlogPostSortOrder`) instead of ignoring the owner sort keys: each
  sort column reads its own keyset index, a cursor is rejected when it is replayed with a
  different sort, and the defaults stay `CREATED_AT DESC`.
- Both Blog admin lists request that sort: the Leptos admin table and the Next admin post
  list expose a sort column and a sort direction control, and changing either restarts the
  list at page one because the keyset cursor is bound to the sort that issued it.
- Blog input validation no longer runs inside a transaction: `create_post` and
  `update_post` validate the post fields and the schedule before `db.begin()`, so a
  rejected request cannot hold a pooled connection.
- `m20260803_000011` reports an unsupported database backend as `DbErr::Custom` instead of
  `unreachable!`, matching the sibling Blog migrations.
- Blog runtime code no longer panics on its SEO/reaction/comments adapter paths: the three
  SEO mappers return errors and their callers propagate them, and the comments snapshot
  reports a policy/read disagreement as `BLOG_COMMENTS_POLICY_UNAVAILABLE`. The three
  remaining `.expect(...)` sites are forced by the SEO and Reactions provider traits, which
  construct a validated key infallibly from a compile-time constant; they are now guarded
  by tests that fail if the constant stops satisfying the owner grammar.
- Blog Next admin `POSTS_QUERY` no longer selects `scheduledAt` twice.
- The wave's own fallout was swept before hand-off: the Blog tests pin the two
  registered listeners by name instead of assuming a single handler, and the removed
  `isPinned` input field no longer appears in `rustok-ai`'s draft handler or in
  `rustok-content-orchestration`'s demote test.
- The hand-off was then verified a second time against the sources: the preview-token service
  no longer describes its TTL rule as clamped (an out-of-range TTL is refused),
  `docs/2026-10-09-blog-draft-preview-tokens.md` matches the current
  `get_post_by_token`/`cleanup_expired` signatures, the implemented
  `blog_preview_token_cleanup` retention and the draft-only accessor, and the
  `src/tests/hardening.rs` harness creates only the host table its paths read
  (`tenant_modules`), because no Blog, Taxonomy or outbox migration declares a foreign key to
  `tenants` and the exercised paths seed no channel.
- The Blog OpenAPI document describes every route the module serves again: `BlogApiDoc`
  registers the three handlers the audit found missing — `preview_post`
  (`GET /api/blog/preview/{token}`), `pin_post` and `unpin_post`
  (`POST /api/blog/posts/{id}/pin|unpin`) — so they reach generated clients through the
  server document that merges the module document
  (`apps/server/src/controllers/swagger.rs`). The pairing is now gated by
  `tests/openapi_document_contract.rs`: the compiled test fails when a handler with a
  `#[utoipa::path]` attribute is absent from `BlogApiDoc`, or when a path the module's
  `axum_router` serves is absent from the generated `openapi_document()`. The text verifier
  set cannot make that assertion — no OpenAPI artifact is committed, so
  `verify-api-surface-contract.mjs` reads the router and the module manifest only.
- Owner-command coverage was added for the slices the audit found untested (L-7):
  `src/tests/owner_commands.rs` exercises `publish_scheduled_posts` (only the tenant's due
  drafts, oldest-due first, the batch clamp, one `blog.post.published` per published post and
  a drained queue on the next iteration) and the pin write path `src/services/post/pin.rs`
  could only cover as a predicate (the flag and its timestamp are written together, archiving
  keeps the pin state and unpin releases both). The scheduled-publish worker's claim is
  covered in `src/services/scheduler.rs`: it selects the tenant with the oldest due schedule,
  ignores a future schedule and another worker's slug, and the claimed item publishes that
  tenant's due draft. The harness's direct schedule write lives in `src/tests/hardening.rs`
  (`set_post_schedule`/`make_post_due`), shared by both test modules.
- A preview token now authorizes a draft read for exactly as long as the post is a draft:
  the lifecycle transition deletes the post's tokens in the same transaction when the post
  leaves `draft`, so publishing (manually or through `blog_scheduled_publish`) and archiving
  both revoke them. The accessor already refused to serve a non-draft post, but a surviving
  row became a live authorization again as soon as `unpublish` or `restore` returned the post
  to draft, and that path is closed. Deleting a post keeps removing its tokens through the
  cascade, and the retention worker is unchanged.

### Added (continued)
- Blog post slug redirects use the shared canonical URL registry of `rustok-content`
  (`canonical_url` / `url_alias`) through `CanonicalUrlWriter`. Renaming a post keeps its old
  route as an alias, public reads resolve it to the current post, a new post may take a retired
  slug, and deleting a post purges its routes. Blog keeps no slug-history table. Content
  orchestration uses the same Blog route definition.
- Blog post slugs for non-ASCII titles are transliterated through the shared Taxonomy route-key
  normalizer, so Cyrillic titles no longer fail with "Slug cannot be empty".
- Page Builder scenario-baseline revision history: every accepted baseline mutation (`create`,
  `replace`, `delete`) appends a record to `page_builder_scenario_baseline_revisions` inside the
  same transaction, and `pageBuilderScenarioBaselineHistory` /
  `PageBuilderScenarioBaselineService::history` read the trail back. The migration that creates the
  table is registered for the first time and applies on next startup; it creates an empty table, so
  there is no backfill and existing baselines are untouched.

### Changed
- Upgraded the embedded `rhai` scripting runtime from 1.26.1 to 1.27.0 (workspace requirement
  moved from `1.25.0` to `1.27.0`) and enabled the `compact_str` feature workspace-wide, so rhai
  identifiers and object-map keys use `CompactString` instead of `SmartString`. The public RusToK
  scripting API is unchanged; the upgrade brings corrected `on_progress` operation counting inside
  native callbacks, `ErrorDataRace` instead of a process panic for shared-value closures under
  `sync`, panic unwind safety for by-value native arguments, and source positions on built-in
  binary-operation errors. Regression coverage in `rustok-sandbox`
  (`src/rhai/regression.rs`) pins the progress-counter, data-race and error-position behaviors.
- Blog `published_at` is the first publication time: republishing, unpublishing, archiving, and
  restoring no longer change or clear it. `updated_at` tracks the last change.
- Blog public list, RSS feed, and related-article reads use the requested locale instead of the
  tenant default locale.
- Blog post SEO fallbacks use `seoTitle`/`seoDescription` first and a localized default description;
  Article JSON-LD adds `dateModified` and an absolute URL.

### Removed
- Blog `view_count` field from the entity, REST/GraphQL-facing DTOs, search projection, and schema
  (migration `m20261008_000031`). The field had no writer and always returned 0.

### Fixed
- Blog `featured_image_url` is validated on create and update: only absolute `http`/`https` URLs or
  root-relative paths, at most 2048 characters. Other schemes were previously stored and reached SEO
  and JSON-LD output.
- Blog RSS feed escapes a literal `]]>` inside CDATA sections, which previously broke the XML.
- The scenario-baseline revision history existed as schema only: the migration was not in the
  `PagesModule` migration list and its entity was not part of the module tree, so the promotion
  trail the schema promised was never written, with or without a reader. The migration is now
  registered as `m20261008_000001_create_scenario_baseline_revision_history` — renamed from the
  `m20260714_000003` it was authored as, because the composed migration plan is name-sorted and
  must stay append-only, and it had never been applied anywhere under the old name. See
  `docs/audits/page-subsystem-engineering-audit-2026-10-08.md` (F-9).

### Deprecated
- _No unreleased deprecations yet._

### Removed
- _No unreleased removals yet._

### Security
- Registered a time-bounded exception for RUSTSEC-2026-0249 (`smartstring` unmaintained, archived
  2026-05-03). It is reached only through rhai; the `compact_str` feature moves the RusToK data
  plane off `SmartString`, and full removal is tracked with rhai 2.0. The waiver in `deny.toml` and
  `.cargo/audit.toml` expires 2027-01-25 and is documented in
  `docs/security/advisory-exceptions.md`.

---

## Release note policy

- Keep this file release-oriented: each released version gets one section
  `## [X.Y.Z] - YYYY-MM-DD` with standard subsections.
- Avoid sprint diaries and implementation logs in this file.
- For deep implementation context, link to canonical docs in `docs/` and `DECISIONS/`.
- Do not reference missing files; every link must resolve inside the repository.

## Version section template

```md
## [X.Y.Z] - YYYY-MM-DD

### Added
- ...

### Changed
- ...

### Fixed
- ...

### Deprecated
- ...

### Removed
- ...

### Security
- ...
```

## Hotspot contract (DOC-12 / H5)

- Hotspot: `H5` (Release and compatibility communication).
- Doc contracts updated: `CHANGELOG.md`.
- Owner scope: platform docs owner.
- Residual drift risk:
  - root onboarding docs (`README.md`, `README.ru.md`) may update earlier
    than release note section in this file;
  - without mandatory release cutover checklist, risk of stale compatibility notes
    remains until complete DOC-12 (B14) closure.
