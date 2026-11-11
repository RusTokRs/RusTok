# rustok-pages

## Purpose

`rustok-pages` owns current Fly-backed pages, localized metadata and bodies,
channel visibility, deterministic published landing artifacts and page routes.

## Responsibilities

- Provide `PagesModule` metadata, permissions and migrations.
- Own page storage across `pages`, `page_translations`, `page_bodies`,
  `page_body_drafts`, `page_body_revisions`, `page_channel_visibility`, scenario
  baselines and landing artifact tables.
- Keep exactly one editable working copy per (page, locale): the page body draft
  while the page is published (the current body keeps serving), the current body
  in every other lifecycle state. Promotion on publish/unpublish/archive folds the
  draft into the current body and preserves its revision token.
- Append every accepted body mutation (`create`, `save`, `draft_save`, `promote`,
  `restore`, `duplicate`) to the append-only `page_body_revisions` journal in the
  mutation's transaction, prune it to the newest `MAX_PAGE_BODY_REVISIONS_PER_BODY`
  rows per (page, locale), and restore journaled revisions under working-copy CAS.
- Duplicate pages into new draft pages: current bodies and channel visibility are
  copied, titles/metadata are kept, and each locale gets a `-copy[-N]` slug suffix.
- Schedule reviewed publications: `page_publish_jobs` captures the exact reviewed
  publish command at scheduling time and a bounded sweep replays due commands
  through `publish_reviewed` as a system actor; claims take a reaper lease, so a
  worker that crashes mid-publish no longer strands its job in `executing` — the
  sweep reaper returns expired leases to `scheduled` (or fails/cancels them; see
  [`DECISIONS/2026-10-09-scheduled-page-publishing.md`](../../../DECISIONS/2026-10-09-scheduled-page-publishing.md)).
- Expose module-owned GraphQL and REST adapters.
- Accept one Page Builder body input through the typed `document` field without
  a caller-selected format or parallel serialized-content field, persist the
  canonical internal `grapesjs` invariant, and use `pages[].component` as the
  component-tree authority.
- Validate builder feature policy and optimistic page revisions.
- Provide the `pages/page_metadata` Translation target through exact locale
  snapshots, owner-local resource/source/target CAS, durable receipts and a
  content-free change cursor.
- Build, persist and serve deterministic immutable landing artifacts.
- Keep the page source and the published content apart on every read. `body` (the
  authoring document) is returned only to a principal that may save or publish that
  document: `pages:update` or `pages:publish` over the page, the same rules as the write
  paths, and the draft of a published page follows the same rule. Public readers,
  customers and read-only staff get `body: null`. Published content is the typed GraphQL
  field `landing { locale html css artifactHash }`, resolved from the artifact bound to
  the page for the effective locale and the request channel, and read through the
  generation-aware Pages cache (see
  [`DECISIONS/2026-11-10-published-page-landing-transport.md`](../../../DECISIONS/2026-11-10-published-page-landing-transport.md)).
- Keep unpublished pages with their authors. The record of a page that is not published
  is readable only with a page authoring permission (`pages:create`, `pages:update`,
  `pages:delete`, `pages:publish`); on GraphQL every other principal, signed in or not,
  is a visitor of the request channel. The by-slug read (`pageBySlug`, REST by slug) is the
  storefront route lookup and resolves published pages only, for an author too; an
  unpublished page is read by id.
- Publish module-owned Leptos admin and storefront packages.
- Enforce `pages:*` permissions in adapters and services.

## Architecture

```text
Pages metadata + Fly body (current copy + unpublished draft)
  -> validation/readiness
  -> deterministic landing renderer
  -> immutable artifact
  -> published artifact binding
  -> storefront route/cache
```

There is no block-based fallback document model, parallel JSON editor or Next
GrapesJS editor. Fresh development databases never create `page_blocks`; no
compatibility or drop migration is retained.

## Interactions

- `rustok-content` supplies shared content status and locale helpers.
- `rustok-page-builder` supplies capability contracts and rollout policy.
- `fly` supplies the current project model, validation and deterministic
  rendering.
- `rustok-channel` supplies channel module gating; Pages owns page-level channel
  visibility.
- `rustok-api` supplies tenant/auth/request contracts.
- `rustok-translation-targets` supplies the neutral owner-provider contract;
  Translation reads and applies Pages metadata only through `PageService`.
- `rustok-core` supplies module contracts and `SecurityContext`.
- `apps/server` composes the module router and GraphQL roots.
- `apps/admin` mounts `rustok-pages-admin::PagesAdmin`.
- `apps/storefront` mounts `rustok-pages-storefront::PagesView`.

## Entry points

- `PagesModule`
- `PageService`
- `PagePublishScheduler`
- `PageBuilderArtifactService`
- `PageBuilderScenarioBaselineService`
- `graphql::PagesQuery`
- `graphql::PagesMutation`
- `controllers::axum_router`
- `rustok-pages-admin::PagesAdmin`
- `rustok-pages-storefront::PagesView`

## Known Limitations / Pending Implementation

Recorded by earlier page-subsystem audits.
These are gaps in what the runtime actually does, not accepted design:

- **Wave 2 scheduled publishing (G-3 of the 2026-10-09 audit) is implemented**: editors
  schedule/cancel a publication (`pagePublishSchedule`, `schedulePagePublish`,
  `cancelPagePublish`; REST `/api/admin/pages/{id}/publish-schedule`; admin schedule
  panel) and a Pages-owned job queue executes due schedules through the idempotent
  `publish_reviewed` command (see
  [`DECISIONS/2026-10-09-scheduled-page-publishing.md`](../../../DECISIONS/2026-10-09-scheduled-page-publishing.md)).
  Behaviour notes: the reviewed command (version, body revision tokens, review hash) is
  captured at scheduling time and replayed unchanged, so content edited after scheduling
  intentionally fails the job with `PAGE_DOCUMENT_REVISION_CONFLICT` instead of publishing
  unreviewed content — reschedule after re-review. Deterministic failures are terminal;
  unknown errors retry up to 5 attempts. Claimed jobs hold a lease
  (`page_publish_jobs.lease_until`, `PAGE_PUBLISH_JOB_LEASE_SECS`): every sweep first
  reaps expired `executing` rows — back to `scheduled` when attempts remain
  (`PAGE_PUBLISH_JOB_LEASE_EXPIRED` records the recovery), terminal `failed` when
  attempts are exhausted, and terminal `canceled` when a newer schedule superseded the
  stranded claim — so a crashed worker cannot strand a publication forever
  (`sweep_due_publish_jobs_as_of` / `reap_expired_publish_jobs_as_of`;
  `PagePublishScheduler::metrics()` exposes `requeued_total` / `reaped_failed_total`
  counters for stranded-state monitoring). Scheduling needs `pages:publish`; execution is a
  system-actor publish whose job row keeps `created_by` attribution. **Hosts must run
  `PagePublishScheduler` (or `process_due_publish_jobs`) for schedules to fire.** Runtime
  coverage lives in `tests/page_publish_scheduler_sqlite.rs` (SQLite); the Postgres suites
  and the full cargo verification matrix remain CI's authority for this change.
- **Wave 1 of the 2026-10-09 audit is implemented** (draft saving on published pages,
  the append-only body revision journal with restore, and page duplication; see
  [`DECISIONS/2026-10-09-page-body-draft-and-revision-history.md`](../../../DECISIONS/2026-10-09-page-body-draft-and-revision-history.md)).
  Behaviour notes that come with it: draft saves on published pages no longer bump
  `pages.updated_at`, so list-level `updated_at` stops reflecting unpublished draft edits;
  inline editing of a published page now commits into its body draft (the live page changes
  only at the next publish or unpublish); restore requires `pages:update`, body
  revision history is readable under the document read rule (`pages:update` or
  `pages:publish` over the page) and never exposes stored body content; duplication copies current
  bodies only and ignores a published source's drafts. Runtime coverage lives in
  `tests/page_body_draft_and_history_sqlite.rs` (SQLite); the Postgres suites and the full
  cargo verification matrix remain CI's authority for this change.
- **Wave 3 site symbols (G-5 of the 2026-10-09 audit) are in progress**: symbol
  definitions live in the shared `site_symbols` catalog (`(tenant, locale, symbol_id)`),
  instances carry `symbolId` and expand at publish, so editing a definition and
  re-publishing re-issues every occurrence on that page
  (see [`DECISIONS/2026-10-09-site-symbols-shared-definitions.md`](../../../DECISIONS/2026-10-09-site-symbols-shared-definitions.md)).
  Behaviour notes: body saves (draft or direct) require a matching `flySymbolsRevision`
  catalog token before replacing stored definitions (definitions are site-wide immediately;
  they have no draft cycle of their own). Restore does not roll back the global catalog.
  Editor reads and reviewed publish merge the stored catalog back (store wins)
  and publish fails closed on unknown references, cycles or a definition snapshot
  newer than the saved page body. Editors must reload, save and re-review a page
  after a shared symbol changes; scheduled publishes also fail closed on drift.
  Already published artifacts update only when their page is re-published.
  `siteSymbolUsage` (GraphQL)
  lists pages to re-issue, including nested symbol dependencies. **Pending pieces:**
  a usage UI, per-instance overrides, bulk re-publish, and a SQLite integration
  suite for the catalog sync (helper-level unit
  coverage lives in `services/page/symbols.rs`; the cargo verification matrix remains CI's
  authority).
- **SEO content values come from the published landing only.** The fallback description
  (used when the translation has no `meta_description`) is the visible text of the landing,
  and the primary image is its first `<img>`. An image that exists only as a CSS background
  is not picked up, and a page without a landing has neither value. An authoring view (the
  SEO editor passes no channel) reads the landing of a channel-restricted page on a channel
  that admits the page; a public route reads it on the request channel only. The record is
  tested against a reviewed publish in `src/seo_target_tests.rs`.
- **Search and the sitemap advertise only pages that have a published landing.** The search
  document of a page locale carries the text of the landing bound to that locale; the page
  body and superseded landings are never indexed. A published page without any bound landing
  has no storefront route and therefore no search document and no sitemap entry. A locale
  without a landing of its own is indexed with the page titles only.
- **There is no read-only access to authoring state.** The page body is returned only to a
  principal that may save or publish the document, and the record of an unpublished page
  only to a principal that holds a page authoring permission (`pages:create`,
  `pages:update`, `pages:delete`, `pages:publish`). `pages:read` and `pages:list` grant
  neither, because public readers and customers hold both; a read-only reviewer role would
  need a permission of its own. No Pages read decides from the role label alone; the label
  only narrows `pages:update` of a non-staff principal to its own pages, as it does for the
  save command.
- **Only the landing of a page read is cached.** GraphQL `landing` and the document of
  `GET /api/pages/{id}/artifact` are read through the generation-aware Pages cache (60 s
  TTL) under one key rule (`published_landing_cache_variant`), so every page event evicts
  both. The page record (status, translation, channel list) is read from the database on
  every request, because it decides what each principal may see.
- **`GET /api/pages/{id}/artifact` is not used by the storefronts of this repository.** It
  serves the complete standalone document over a cacheable `GET` for iframes, CDNs and
  external clients; both storefronts embed `landing` instead. The server answers from no
  entry older than the last page event, but a downstream cache may keep the document for
  the freshness the response declares (`max-age=60, stale-while-revalidate=300`), also
  after the page was unpublished or hidden on a channel.
- **REST `GET /api/pages` is not a storefront transport.** It requires a token and returns
  the page record without a landing. A token without a page authoring permission reads as
  a visitor of the request channel, as on GraphQL.
- **A published page without a landing has no storefront route.** The Next route answers
  404 and the Leptos host route decision answers `not_found`. The production publish
  command always binds a landing; only the non-builder service lifecycle
  (`publish_non_builder`) produces such a page.
- **Write commands answer with the caller's read.** Create, document save, metadata patch
  and lifecycle commands return the page as the calling principal may read it, so a
  principal that may create but neither save nor publish gets its own page back without
  the body and with `document_readable: false`. This is the rule, not a gap: no response
  carries more than a read by the same principal would.
- **Draft state has no dedicated signal in the page list.** `PageBody.state` reports
  `draft` vs `current` on page detail reads only; the admin document list cannot show at a
  glance which published pages carry unpublished draft edits.
- **Only builder-format documents get drafts.** `save_document` accepts the Fly/GrapesJS
  body format only (as before), so the draft working copy exists for exactly the documents
  the builder edits; non-builder page content follows the metadata/translation write paths.
- **Body revision restore is locale-scoped and time-scoped.** Restore targets one (page,
  locale) working copy from its journal; there is no whole-page point-in-time restore, and
  rows pruned by the 50-per-(page, locale) retention window are gone.

- **The scenario-baseline revision history has no retention policy.** Every accepted baseline
  mutation (`create`, `replace`, `delete`) appends a row to
  `page_builder_scenario_baseline_revisions` (created by
  `m20261008_000001_create_scenario_baseline_revision_history`, the renamed, previously unregistered
  `m20260714_000003`) in the same transaction as the mutation, and
  `PageBuilderScenarioBaselineService::history` /
  `pageBuilderScenarioBaselineHistory` read it back (newest first, capped at 200). Nothing prunes
  the journal, so it grows with the page's review activity; a retention window is a product
  decision that has not been made.
- **The admin UI does not surface the revision history.** `rustok-pages-admin` reads the current
  baseline and the scenario release status only, so the trail is reachable through GraphQL and SQL
  rather than in the editor.
- **Reviewed publish compiles on the request task and reads its inputs twice.** The scenario
  gate and the compilation are first run before the publish transaction so the page lock is
  not held during them, and the transaction then re-reads the same inputs to confirm the
  result (`precompiled_evaluation` in
  `rustok-page-builder/contracts/page-builder-publish-runtime-review.json`). The work still
  runs on the async task, not on a blocking pool; symbol and layout expansion still runs under
  the lock; when the inputs change in between, the gate and the compilation run a second time
  under the lock. Lock-hold time has not been measured.
- **Scenario baselines captured before 2026-10-08 are migrated on read, not on deploy.** Their
  integrity hash was FNV-1a 64 and is rewritten to a SHA-256 digest the first time the row is read
  (`upgrade_legacy_baseline_hashes`). A row that is never read keeps the retired hash; that is
  deliberate, and verification still accepts the retired form so nothing is retroactively rejected.

## Docs

- [Public Contract Reference (CRATE_API.md)](./CRATE_API.md)
- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)

