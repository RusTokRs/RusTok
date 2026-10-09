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
  through `publish_reviewed` as a system actor (see
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

Recorded by the 2026-10-08 page-subsystem audit
([`docs/audits/page-subsystem-engineering-audit-2026-10-08.md`](../../../docs/audits/page-subsystem-engineering-audit-2026-10-08.md))
and the 2026-10-09 functional audit
([`docs/audits/pages-page-builder-functional-audit-2026-10-09.md`](../../../docs/audits/pages-page-builder-functional-audit-2026-10-09.md)).
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
  unknown errors retry up to 5 attempts. Scheduling needs `pages:publish`; execution is a
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
  only at the next publish or unpublish); body revision history and restore require
  `pages:update` and history never exposes stored body content; duplication copies current
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
  and publish fails closed on unknown references or cycles; already published artifacts
  update only when their page is re-published. `siteSymbolUsage` (GraphQL)
  lists pages to re-issue, including nested symbol dependencies. **Pending pieces:**
  a usage UI, per-instance overrides, bulk re-publish, and a SQLite integration
  suite for the catalog sync (helper-level unit
  coverage lives in `services/page/symbols.rs`; the cargo verification matrix remains CI's
  authority).
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
- **Scenario baselines captured before 2026-10-08 are migrated on read, not on deploy.** Their
  integrity hash was FNV-1a 64 and is rewritten to a SHA-256 digest the first time the row is read
  (`upgrade_legacy_baseline_hashes`). A row that is never read keeps the retired hash; that is
  deliberate, and verification still accepts the retired form so nothing is retroactively rejected.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
