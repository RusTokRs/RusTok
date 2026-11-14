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

Pages owns an exact-locale, revision-guarded layout catalog. Non-default layouts
compose their header and footer symbols into the page body for builder preview,
review and reviewed publish; published artifacts remain immutable. The Leptos
admin manages layout symbol IDs, but is not a full visual layout editor. Authenticated
`pages:create` principals can list usable exact-locale layout keys; without
`pages:manage` only virtual `default` is returned, because builder composition of
non-default layouts currently requires that grant. Full definitions and authoring
remain `pages:manage`-only. Editors without it cannot compose existing non-default
pages in the builder; a page-scoped preview authorization path remains open. Both create
forms check the chosen key against the catalog and recheck immediately before
creation, failing visibly on lookup errors. Both request opt-in `checkedLayout`
creation: for a single translation it checks exact tenant/locale existence while
serializing with same-locale deletion on PostgreSQL/SQLite; MySQL non-default selection
fails closed. SQLite contention can fail with a busy error. It does not pin the layout
revision and does not cover legacy create/metadata free-form labels or cross-locale
deletion. The server must be deployed before clients send the new input field. Reviewed
publish must resolve non-default layouts for every page locale. The new database and
concurrency behavior has not yet been verified in this environment.
See the [page layout decision](../../../../DECISIONS/2026-10-09-page-layouts-and-menu-page-links.md).
Site symbols still require manual re-review and re-publication of dependent
pages after a shared edit.

Page starters are separate from layouts. The backend catalog combines built-ins
and tenant records, with tenant keys overriding built-ins before category
filtering. Save rejects documents that cannot be cloned, and instantiation
reissues identities. Both admin create forms select from that server catalog and
request instantiation at the observed revision and page locale; template text is not rewritten
with the page title. Embedded site-symbol catalogs are never imported as authority:
linked starters use the current exact-locale catalog and require `pages:manage`,
while unlinked starters discard embedded definitions. Missing references and stale
catalog revisions fail instead of replacing current site symbols. Both admin UIs can
create a new starter from a page's saved exact-locale working body, explicitly excluding
unsaved editor changes. The returned state and revision identify the captured body;
source selection and insertion are not revision-locked as one transaction. This flow
cannot replace existing or built-in keys. Both admin UIs also provide tenant-only
metadata editing and confirmed deletion with observed revision tokens; metadata
updates leave the document and thumbnail untouched. Built-ins are read-only and
deleting an override reveals its built-in. Automatic application of suggested
layouts remains open. Tenant starter saves and deletes use opaque revision tokens; deleting an
override reveals its built-in. `default_template_key` is syntax-checked, not
resolved for a locale. Old writers must be stopped during the CAS migration.

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
