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

- **The scenario-baseline revision history has no retention policy.** Every accepted baseline
  mutation (`create`, `replace`, `delete`) appends a row to
  `page_builder_scenario_baseline_revisions` in the same transaction as the mutation, and
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
