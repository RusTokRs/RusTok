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

The audit is source-only. No verifier, formatter, Cargo command, browser scenario,
database scenario, workflow or CI run is claimed by this update.

## Corrected parity snapshot

### Rollback control: source-connected

The Pages admin already mounts `PagesRollbackControl`. It loads the selected page,
shows only for published pages, requires a prepare/confirm interaction, delegates to
the typed `rollback_page` transport, consumes the returned typed result version and
refreshes the workspace after a successful receipt. The older Pages plan item saying
the rollback action still needs to be added is stale.

This source connection does not promote execution evidence. Accepted rollback,
outbox, cache-generation and storefront refill packets remain open.

### Typed metadata contribution: source-connected

Pages already registers `rustok.pages.metadata`, provides six typed fields, supplies
a `ConsumerPropertyEditorRuntime`, loads through `fetch_page`, saves through
`patch_page_metadata`, binds optimistic concurrency to
`pages:{page_id}:metadata:v{version}` and never writes the Fly document. The older
Pages plan statement that a typed metadata-only contribution still needs to be added
is stale.

The current source still contains the bespoke `PageMetadataEditor`, so the overall
metadata UI cutover is not complete.

### Standalone consumer-property surface: source-ready

The canonical `ConsumerPropertiesPanel` is now a public Page Builder admin component.
It accepts only the framework-neutral `ConsumerPropertyEditorRuntime` and the exact
`ContributionAssemblyResult`, verifies contribution/schema identity before loading,
and delegates load/save through the consumer-owned port.

The component no longer requires callers to mount the Fly canvas. This removes the
provider-side blocker for rendering the same registered metadata property surface in
a Pages-owned published-document workspace while keeping the published Fly document
unmounted and immutable.

### Legacy PageMetadataEditor: pending removal

`PageMetadataEditor` remains mounted in `crates/rustok-pages/admin/src/composition.rs`.
It duplicates the same metadata fields and owner transport already represented by the
registered consumer-property contract. It must remain explicitly open until Pages
replaces it for both draft and published metadata views.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Evidence state |
| --- | --- | --- | --- | --- |
| Metadata schema and values | Pages | Contract/runtime validation | Connected | Pending execution |
| Metadata optimistic revision | Pages | Typed save envelope | Connected | Pending conflict packet |
| Draft metadata panel inside Fly | Pages runtime/port | Leptos consumer panel | Connected | Pending execution |
| Published metadata without Fly canvas | Pages host composition | Standalone consumer panel | Provider seam ready; Pages cutover pending | Pending |
| Immutable artifact rollback action | Pages | No lifecycle ownership | Connected | Pending rollback/cache packet |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Pending observed packet |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Pending bundle/runtime proof |

## Changes in this slice

1. Export `ConsumerPropertiesPanel` from the Page Builder admin crate as a reusable
   standalone Leptos surface.
2. Preserve exact contribution-schema verification, optimistic snapshot/save receipt
   behavior and consumer-owned persistence.
3. Actualize the machine-readable consumer-property contract with the standalone
   host surface and explicit legacy-form dependency.
4. Strengthen the Pages metadata source guard to require both public exports and this
   parity plan while keeping the legacy form explicitly pending.

## Boundaries

This slice does not:

- remove or replace `PageMetadataEditor`;
- change Pages metadata persistence, DTOs, GraphQL, HTTP or browser transport;
- mount an editable Fly document for published pages;
- change Page Builder capability authorization, rollout policy or contribution
  assembly semantics;
- alter publish, unpublish, rollback, artifact, cache or storefront behavior;
- add Dioxus or mobile rendering;
- claim executed evidence or test results.

## Next cursor

1. Replace the bespoke Pages metadata form with the exported registered
   `ConsumerPropertiesPanel` for both draft and published metadata workspaces.
2. Build the Pages-owned contribution assembly for the standalone host path and pass
   the already-provided metadata runtime without introducing a second owner port.
3. Keep the Fly canvas absent for published pages and preserve the immutable published
   artifact boundary.
4. Update the machine contract and verifier from `legacy_form_pending` to the final
   cutover state only after `PageMetadataEditor` is absent from source.
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
