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
- The page body is authoring state: reads return it only to a principal that may
  save or publish the document. Storefronts render the published landing (`landing` in
  GraphQL, the bound artifact in the native adapter) and have no fallback
  content source. A published page without a landing has no storefront route.
- Every page read says whether the body was withheld: `document_readable`
  (`documentReadable` in GraphQL) is `false` when the principal may not read the
  document, and `true` with an absent body when the page has no document in the
  effective locale. Write commands answer with the caller's read, so the same
  rule applies to their responses. The Leptos admin opens the editor only when
  the field is `true`.
- SEO values that describe page content (the fallback description, the primary
  image) are derived from the published landing, never from the page body.
- The search document of a page carries the text of the landing bound to the
  page locale. A published page without a bound landing has no search document
  and no sitemap entry.
- The revision journal of a document is readable under the document read rule
  (`pages:update` or `pages:publish` over the page); restoring a revision
  requires `pages:update`.
- An unpublished page exists only for principals that hold a page authoring
  permission; any other principal is a visitor of the request channel, on
  GraphQL and on REST.
- The published landing is cached once, on the server, in the generation-aware
  Pages cache. The GraphQL `landing` entry and the REST landing document entry
  share one key rule and rotate with every page event; storefront applications
  keep no cache of their own.
- These rules are recorded in the [published page landing transport
  decision](../../../../DECISIONS/2026-11-10-published-page-landing-transport.md).

## Known Limitations

The `pages.template` field is presently a free-form label; no stored layout
wraps the document during review/publish. The accepted [page layout and menu
link decision](../../../../DECISIONS/2026-10-09-page-layouts-and-menu-page-links.md)
specifies the Pages-owned versioned template and host-resolved menu target.
The additive catalog, revision-guarded service and GraphQL authoring operations
exist. `previewPageTemplateDocument` produces a sanitized, non-persistent
composition of a supplied document and its exact-locale layout. The editor does
not yet call this preview; templates are **not used by reviewed publish**:
selecting a `pages.template` string does not change the served page. There is
no layout editor, existing-label cutover, menu page-target writer or host route
adapter.
Site symbols still require manual re-review and
re-publication of dependent pages after a shared edit.

## Integration

- `rustok-content` supplies content status and locale helpers.
- `rustok-page-builder` supplies FBA capability and rollout contracts.
- `fly` supplies current document validation and deterministic rendering.
- `rustok-channel` supplies module-level channel gating; Pages owns page-level
  visibility.
- `rustok-navigation` owns menus and menu-item localization; Pages may compose
  Navigation public contracts without owning its storage.
- host applications connect module UI through generated manifest composition.

## Route query boundedness

Route resolution and immutable route-history recording batch tenant-scoped reads. The current published-route resolver uses one candidate-translation query plus one published-page `IN` query; publication snapshots and delete tombstones preload their existing route rows before per-row decision logic. Historical route semantics remain fail-closed on duplicate or conflicting rows.

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
- [Event flow contract](../../../../docs/architecture/event-flow-contract.md)

### Immutable Artifact Integrity & Repair Specifications

- [Artifact Repair Rollback Continuity](./artifact-repair-rollback-continuity.md)
- [Explicit Artifact Repair Transports](./explicit-artifact-repair-transports.md)
- [Explicit Immutable Artifact Binding Replacement](./explicit-immutable-artifact-binding-replacement.md)
- [Explicit Immutable Artifact Loss Activation Recovery](./explicit-immutable-artifact-loss-activation-recovery.md)
- [Explicit Immutable Artifact Rebuild](./explicit-immutable-artifact-rebuild.md)
- [Immutable Artifact Integrity Audit Transport](./immutable-artifact-integrity-audit-transport.md)
- [Immutable Artifact Integrity Audit](./immutable-artifact-integrity-audit.md)
- [Immutable Artifact Rebuild Provenance](./immutable-artifact-rebuild-provenance.md)
