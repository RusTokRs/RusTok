# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-03
Status: source-cutover-complete / execution-evidence-pending
Scope: `rustok-pages` admin FFA and `rustok-page-builder` consumer-property surface

## Audit basis

This plan reconciles the current source with:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `crates/rustok-page-builder/contracts/page-builder-consumer-properties.json`;
- the Pages admin composition, rollback control and metadata owner port;
- the Page Builder consumer-property runtime and Leptos panel.

The audit remains source-only. No verifier, formatter, Cargo command, browser
scenario, database scenario, workflow or CI run is claimed by this update.

## Corrected parity snapshot

### Rollback control: source-connected

The Pages admin mounts `PagesRollbackControl`. It loads the selected page, renders
only for published pages, requires prepare/confirm, delegates to the typed rollback
transport, consumes the typed result version and refreshes the workspace after a
successful receipt.

Accepted rollback, outbox, cache-generation and storefront refill packets remain
open.

### Typed metadata contribution: source-connected

Pages registers `rustok.pages.metadata` with six typed fields. The owner runtime
loads through the current page read, saves through `patch_page_metadata`, binds
optimistic concurrency to `pages:{page_id}:metadata:v{version}` and never writes the
Fly document.

### Draft registered metadata surface: source-connected

Draft workspaces render the canonical `ConsumerPropertiesPanel` inside the Fly
properties column. The panel verifies the exact contribution identity and schema
before loading and saving through the Pages-owned runtime and port.

Draft document mutation remains owned by the Pages builder facade. Metadata saves use
the independent page metadata version and cannot write the Fly project.

### Published registered metadata surface: source-connected

`PagesPublishedMetadataSurface` composes the same exported
`ConsumerPropertiesPanel` for the selected published page. It reuses the provided
runtime, builds the exact Pages contribution assembly and does not mount
`PageBuilderAdmin`, `PagesBuilderFacade` or an editable Fly canvas.

The published artifact remains immutable while its independent metadata uses the
same registered contract as the draft editor.

### Legacy PageMetadataEditor: removed

The bespoke `PageMetadataEditor` call and component are absent from
`crates/rustok-pages/admin/src/composition.rs`. Its direct
`transport::patch_page_metadata` UI path is also absent.

Draft metadata is now reachable only through the registered Fly properties surface.
Published metadata is reachable only through the registered standalone surface.
Persistence remains owned by `PagesMetadataPropertyPort`; no second runtime or owner
port was introduced.

### Metadata UI cutover: source-complete

The source now has one registered metadata contract and two lifecycle-specific host
placements:

- draft: canonical panel inside Fly;
- published: canonical panel outside Fly, with the immutable document unmounted.

The machine contract and source guard treat the legacy editor as removed. Execution
evidence remains pending.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Evidence state |
| --- | --- | --- | --- | --- |
| Metadata schema and values | Pages | Contract/runtime validation | Connected | Pending execution |
| Metadata optimistic revision | Pages | Typed save envelope | Connected | Pending conflict packet |
| Draft metadata panel inside Fly | Pages runtime/port | Leptos consumer panel | Connected | Pending execution |
| Published metadata without Fly canvas | Pages shell composition | Standalone consumer panel | Connected | Pending execution |
| Legacy metadata form | None | None | Removed | Not applicable |
| Immutable artifact rollback action | Pages | No lifecycle ownership | Connected | Pending rollback/cache packet |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Pending observed packet |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Pending bundle/runtime proof |

## Changes in this slice

1. Remove the duplicated `PageMetadataEditor` invocation and implementation.
2. Remove the direct metadata persistence call from the Pages workspace composition.
3. Preserve draft metadata through the existing Fly properties panel.
4. Preserve published metadata through `PagesPublishedMetadataSurface`.
5. Promote the machine contract to `metadata_surface_cutover_complete`.
6. Strengthen the source guard to require the old component, invocation and direct
   persistence path to remain absent.
7. Actualize this parity plan while keeping every execution claim open.

## Boundaries

This slice does not:

- change Pages metadata persistence, DTOs, GraphQL, HTTP or browser transport;
- add a second metadata runtime or owner port;
- mount an editable Fly document for published pages;
- change Page Builder capability authorization, rollout policy or contribution
  assembly semantics;
- alter publish, unpublish, rollback, artifact, cache or storefront behavior;
- add Dioxus or mobile rendering;
- claim executed evidence or test results.

## Next cursor

1. Retain an accepted metadata packet proving exact stale-revision conflict behavior.
2. Retain an accepted dirty-Fly isolation packet proving metadata save cannot mutate,
   reset or replace an unsaved Fly document.
3. Retain a published metadata packet proving the Fly canvas remains unmounted while
   the registered property save advances only the metadata version.
4. Retain publish/rollback cache packets correlating receipts, outbox events, handler
   receipts, generation rotation and storefront/artifact misses and refills.
5. Complete compile, browser, workflow and rollout evidence before promoting FFA/FBA
   status beyond source-connected.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
cargo check -p rustok-page-builder-admin --all-targets
cargo check -p rustok-pages-admin --all-targets
```
