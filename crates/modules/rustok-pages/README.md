# rustok-pages

## Purpose

`rustok-pages` owns current Fly-backed pages, localized metadata and bodies,
channel visibility, deterministic published landing artifacts and page routes.

## Responsibilities

- Provide `PagesModule` metadata, permissions and migrations.
- Own page storage across `pages`, `page_translations`, `page_bodies`,
  `page_channel_visibility`, scenario baselines and landing artifact tables.
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
Pages metadata + current Fly body
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
- `PageBuilderArtifactService`
- `PageBuilderScenarioBaselineService`
- `graphql::PagesQuery`
- `graphql::PagesMutation`
- `controllers::axum_router`
- `rustok-pages-admin::PagesAdmin`
- `rustok-pages-storefront::PagesView`

## Known Limitations / Pending Implementation

Recorded by the 2026-10-08 page-subsystem audit
([`docs/audits/page-subsystem-engineering-audit-2026-10-08.md`](../../../docs/audits/page-subsystem-engineering-audit-2026-10-08.md)).
These are gaps in what the runtime actually does, not accepted design:

- **The scenario-baseline revision history is not written.** Migration
  `m20260714_000003_create_scenario_baseline_revision_history` creates
  `page_builder_scenario_baseline_revisions`, and
  `src/entities/page_builder_scenario_baseline_revision.rs` models it — but that file is not
  declared in `src/entities/mod.rs`, so it is not part of the crate's module tree at all, and
  nothing in the workspace inserts into or reads the table.
  `PageBuilderScenarioBaselineService` updates the active row and its `previous_baseline_hash`
  column and stops there. The schema therefore promises a promotion trail that no code produces,
  and `previous_baseline_hash` carries exactly one previous value rather than a history. Either
  declare and write the revision on save/replace/delete (with the promotion note and actor that
  `save_if_current` already carries) and read it back, or remove the orphan entity and the table.
- **Scenario baselines captured before 2026-10-08 are migrated on read, not on deploy.** Their
  integrity hash was FNV-1a 64 and is rewritten to a SHA-256 digest the first time the row is read
  (`upgrade_legacy_baseline_hashes`). A row that is never read keeps the retired hash; that is
  deliberate, and verification still accepts the retired form so nothing is retroactively rejected.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../../docs/index.md)
