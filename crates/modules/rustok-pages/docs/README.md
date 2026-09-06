# `rustok-pages` Documentation

`rustok-pages` is the domain module for Pages metadata, current Fly documents,
channel visibility and deterministic published artifacts.

## Purpose

- publish the canonical Pages runtime contract;
- keep persistence, transport adapters and UI packages module-owned;
- provide one current visual-document model without fallback editors or block
  storage;
- remain tenant- and channel-aware without reverting to shared node storage.

## Scope

- `PageService`, `PageBuilderArtifactService` and
  `PageBuilderScenarioBaselineService`;
- storage for pages, translations, bodies, channel visibility, scenario
  baselines and immutable landing artifacts;
- language-agnostic base rows with normalized `VARCHAR(32)` parallel locale records,
  tenant-composite ownership and one effective locale per response;
- exact `pages/page_metadata` Translation provider snapshots and owner-local
  application for title, review-only slug, meta title and meta description;
- per-locale revisions and a content-free owner change journal; Translation
  never receives direct Pages table-write access;
- GraphQL/REST adapters and Leptos admin/storefront packages;
- canonical Fly writes carry one typed `document` field and select the Page
  Builder format server-side; callers do not submit a body format or a parallel
  serialized-content field;
- deterministic publish/build/integrity and storefront artifact delivery;
- typed permission, feature-gate, revision and artifact-integrity failures.

## Current-only rules

- `pages[].component` is the component-tree authority.
- `page_blocks`, `PageBlock`, `BlockService` and block mutations are removed.
- The old Next/GrapesJS editor and parallel JSON/CRUD editor are not supported.
- Page Builder supplies capability contracts and Fly runtime primitives; Pages
  owns metadata, persistence, lifecycle, routing and artifact selection.
- Missing providers and invalid documents fail visibly rather than falling back
  to another document model.

## Integration

- `rustok-content` supplies content status and locale helpers.
- `rustok-page-builder` supplies FBA capability and rollout contracts.
- `fly` supplies current document validation and deterministic rendering.
- `rustok-channel` supplies module-level channel gating; Pages owns page-level
  visibility.
- `rustok-navigation` owns menus and menu-item localization; Pages may compose
  Navigation public contracts without owning its storage.
- host applications connect module UI through generated manifest composition.

## Verification

- `cargo xtask module validate pages`
- `cargo xtask module test pages`
- `cargo test -p rustok-pages`
- `cargo test -p rustok-pages-admin`
- `cargo test -p rustok-pages-storefront`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:fba:baseline`
- Pages no-legacy/no-block source guardrails

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin package](../admin/README.md)
- [Storefront package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
