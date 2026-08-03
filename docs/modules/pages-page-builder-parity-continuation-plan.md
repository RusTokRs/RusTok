# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-03
Status: active
Scope: `rustok-pages` admin FFA and `rustok-page-builder` consumer-property surface

## Audit basis

This continuation plan reconciles the current source with:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `crates/rustok-page-builder/contracts/page-builder-consumer-properties.json`;
- the Pages admin composition, rollback control and metadata owner port;
- the Page Builder consumer-property runtime and Leptos panel.

The audit remains source-only. No verifier, formatter, Cargo command, browser
scenario, database scenario, workflow or CI run is claimed by this update.

## Corrected parity snapshot

### Rollback control: source-connected

The Pages admin mounts `PagesRollbackControl`. It loads the selected page, shows only
for published pages, requires a prepare/confirm interaction, delegates to the typed
`rollback_page` transport, consumes the returned typed result version and refreshes
the workspace after a successful receipt. The older Pages plan item saying the
rollback action still needs to be added is stale.

Accepted rollback, outbox, cache-generation and storefront refill packets remain
open.

### Typed metadata contribution: source-connected

Pages registers `rustok.pages.metadata`, provides six typed fields, supplies a
`ConsumerPropertyEditorRuntime`, loads through `fetch_page`, saves through
`patch_page_metadata`, binds optimistic concurrency to
`pages:{page_id}:metadata:v{version}` and never writes the Fly document. The older
Pages plan statement that a typed metadata-only contribution still needs to be added
is stale.

### Draft registered metadata surface: source-connected

Draft workspaces already render the canonical `ConsumerPropertiesPanel` inside the
Fly properties column. The panel verifies the exact contribution identity and schema
before loading and saving through the Pages-owned runtime and port.

Draft document mutation remains owned by the Pages builder facade. Metadata saves use
the independent page metadata version and cannot write the Fly project.

### Published registered metadata surface: source-connected

`PagesPublishedMetadataSurface` now composes the same exported
`ConsumerPropertiesPanel` in the Pages shell for the selected published page. It:

- reuses the already-provided `ConsumerPropertyEditorRuntime`;
- builds the exact Pages contribution assembly from
  `pages_admin_contribution_policy`;
- loads the current selected page and renders only when its status is `published`;
- shares the existing workspace refresh generation after a successful typed save;
- does not construct a second owner port or call metadata persistence directly;
- does not mount `PageBuilderAdmin`, `PagesBuilderFacade` or an editable Fly canvas.

The published artifact therefore remains immutable while its independent metadata
surface uses the same registered property contract as the draft editor.

### Legacy PageMetadataEditor: pending duplicate removal

`PageMetadataEditor` remains mounted in `crates/rustok-pages/admin/src/composition.rs`.
It now duplicates registered metadata surfaces that are source-connected for both
draft and published states. Removal remains explicit because the current GitHub
contents write path cannot safely apply a small structural deletion to that large
workspace file without replacing the complete file.

No final cutover or absence claim is made in this slice.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Evidence state |
| --- | --- | --- | --- | --- |
| Metadata schema and values | Pages | Contract/runtime validation | Connected | Pending execution |
| Metadata optimistic revision | Pages | Typed save envelope | Connected | Pending conflict packet |
| Draft metadata panel inside Fly | Pages runtime/port | Leptos consumer panel | Connected | Pending execution |
| Published metadata without Fly canvas | Pages shell composition | Standalone consumer panel | Connected | Pending execution |
| Legacy metadata form | Pages composition | None | Duplicate; removal pending | Not applicable |
| Immutable artifact rollback action | Pages | No lifecycle ownership | Connected | Pending rollback/cache packet |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Pending observed packet |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Pending bundle/runtime proof |

## Changes in this slice

1. Add `PagesPublishedMetadataSurface` as a Pages-owned published-only composition.
2. Reuse the canonical exported `ConsumerPropertiesPanel`, existing metadata runtime
   and Pages contribution assembly without a second persistence path.
3. Mount the published registered surface beside the existing rollback/workspace
   shell and bind it to the shared refresh generation.
4. Actualize the machine-readable contract with separate draft and published surface
   states while retaining the legacy duplicate as open.
5. Strengthen the source guard to require published-only selection, no Fly host,
   exact runtime/assembly reuse and no direct metadata write.

## Boundaries

This slice does not:

- remove or edit `PageMetadataEditor`;
- change Pages metadata persistence, DTOs, GraphQL, HTTP or browser transport;
- add a second metadata runtime or owner port;
- mount an editable Fly document for published pages;
- change Page Builder capability authorization, rollout policy or contribution
  assembly semantics;
- alter publish, unpublish, rollback, artifact, cache or storefront behavior;
- add Dioxus or mobile rendering;
- claim executed evidence or test results.

## Next cursor

1. Remove the now-duplicated bespoke `PageMetadataEditor` call and component from
   `crates/rustok-pages/admin/src/composition.rs` through a checkout-capable patch
   path rather than a full-file contents replacement.
2. Preserve draft metadata through the existing Fly properties panel and published
   metadata through `PagesPublishedMetadataSurface`.
3. Update the machine contract and verifier from
   `published_surface_connected_legacy_form_pending` to the final cutover state only
   after both `fn PageMetadataEditor` and `<PageMetadataEditor` are absent.
4. Update the canonical Pages and Page Builder implementation plans to mark rollback,
   typed metadata contribution and registered draft/published metadata surfaces as
   completed while keeping execution evidence open.
5. Retain an accepted metadata packet proving exact revision-conflict behavior and
   that metadata save cannot mutate or replace a dirty Fly document.
6. Retain accepted publish/rollback cache packets correlating receipts, outbox events,
   handler receipts, generation rotation and storefront/artifact misses and refills.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
cargo check -p rustok-page-builder-admin --all-targets
cargo check -p rustok-pages-admin --all-targets
```
