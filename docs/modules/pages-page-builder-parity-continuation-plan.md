# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-03
Status: source-cutover-complete / published-surface-regression-ready / execution-evidence-pending
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

Draft metadata is reachable only through the registered Fly properties surface.
Published metadata is reachable only through the registered standalone surface.
Persistence remains owned by `PagesMetadataPropertyPort`; no second runtime or owner
port was introduced.

### Metadata UI cutover: source-complete

The source has one registered metadata contract and two lifecycle-specific host
placements:

- draft: canonical panel inside Fly;
- published: canonical panel outside Fly, with the immutable document unmounted.

The machine contract and source guard treat the legacy editor as removed.

### Metadata revision/isolation source packet: ready, unvalidated

`PagesMetadataPropertyPort` separates production transport from metadata command
preparation through a private `PagesMetadataTransport` seam. The production adapter
still delegates to the same `fetch_page` and `patch_page_metadata` calls.

The save path has an explicit, guarded order:

1. validate the registered contribution and exact field set;
2. parse the page-scoped metadata revision;
3. read the current page;
4. require the current page version to equal the expected metadata version;
5. construct a metadata-only transport request;
6. call the metadata patch transport;
7. require the returned page version to advance;
8. publish only `PageMutationResult` to the owner callback.

The focused stale metadata revision short-circuits before patch transport and returns
the exact stable code `REVISION_CONFLICT`.

The metadata-only transport request contains token, tenant, page identity, expected
metadata version, locale and the six registered metadata values. It contains no Fly
body, `content_json`, project data, controller, document revision or Page Builder
command.

The regression harness retains an unsaved dirty Fly sentinel beside a successful
metadata save. The dirty Fly state is not accepted by the metadata owner port and is
asserted byte-for-byte unchanged after the metadata receipt advances from metadata
version 7 to 8.

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-metadata-revision-isolation-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs`;
- the focused tests in `crates/rustok-pages/admin/src/metadata_properties.rs`.

Execution evidence remains pending.

### Published metadata source packet: ready, unvalidated

The standalone published surface now uses one closed admission state:

- a selected page whose status equals `published` case-insensitively admits the
  registered metadata panel;
- draft, archived, empty-status and missing-page states remain hidden.

The source regressions retain this published-only admission policy without requiring a
browser or network transport. The production surface still reads the selected page,
reuses the existing `ConsumerPropertyEditorRuntime`, builds the Pages contribution
assembly and renders the canonical `ConsumerPropertiesPanel`.

The surface publishes a stable DOM contract for a future browser packet:

- `data-pages-published-metadata-surface="registered"`;
- `data-pages-published-metadata-admission="published-only"`;
- `data-pages-fly-canvas-mounted="false"`;
- `data-pages-document-authoring="false"`;
- `data-pages-metadata-runtime="registered"`;
- `data-pages-metadata-persistence="owner-port"`.

The source and guard forbid a Pages builder facade, Page Builder host context, Fly
builder, direct metadata patch call, document save or local runtime provisioning in
the standalone surface. Persistence remains delegated to the existing metadata owner
port, whose independent revision and dirty-Fly isolation packet is linked by the new
evidence contract.

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs`;
- the focused tests in `crates/rustok-pages/admin/src/standalone_metadata.rs`.

Browser execution remains pending. The DOM markers are selectors for a retained
browser packet; they are not a claim that the browser scenario ran.

### Canonical plans: actualized to source parity

The Pages and Page Builder canonical implementation plans mark the typed rollback
control, registered metadata contribution, draft/published panel composition and
legacy editor removal as source-complete. They list the focused conflict/isolation
regressions as source-ready while keeping every executed packet, browser proof,
workflow check and rollout gate open.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Evidence state |
| --- | --- | --- | --- | --- |
| Metadata schema and values | Pages | Contract/runtime validation | Connected | Source regression ready; execution pending |
| Metadata optimistic revision | Pages | Typed save envelope | Connected | Conflict regression ready; execution pending |
| Dirty Fly isolation during metadata save | Pages metadata port | Fly/Page Builder controller | Source-guarded | Isolation regression ready; execution pending |
| Draft metadata panel inside Fly | Pages runtime/port | Leptos consumer panel | Connected | Pending browser execution |
| Published metadata without Fly canvas | Pages shell composition | Standalone consumer panel | Admission and DOM source-guarded | Browser execution pending |
| Legacy metadata form | None | None | Removed | Not applicable |
| Immutable artifact rollback action | Pages | No lifecycle ownership | Connected | Pending rollback/cache packet |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Pending observed packet |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Pending bundle/runtime proof |

## Changes in this slice

1. Extract one deterministic published metadata surface admission policy.
2. Add focused source regressions for published, uppercase published, draft,
   archived, empty-status and missing-page states.
3. Add stable DOM markers describing the registered panel, published-only admission,
   absent Fly canvas, absent document authoring, reused runtime and owner-port
   persistence.
4. Add a machine-readable published-surface source evidence contract.
5. Add a focused static verifier that links published admission to the existing
   metadata revision/isolation evidence.
6. Register the packet in the Page Builder consumer-properties machine contract and
   the shared metadata source guard.
7. Actualize this parity plan while keeping every execution claim open.

## Boundaries

This slice does not:

- change Pages metadata DTOs, GraphQL, HTTP or browser transport;
- change the public `ConsumerPropertyEditorRuntime` contract;
- add a second metadata runtime or owner port;
- mount an editable Fly document for published pages;
- change Page Builder capability authorization, rollout policy or contribution
  assembly semantics;
- alter publish, unpublish, rollback, artifact, cache or storefront behavior;
- add a browser harness, Dioxus or mobile rendering;
- claim executed evidence or test results.

## Next cursor

1. Run and retain the focused metadata conflict and dirty-Fly isolation packets.
2. Run and retain the published metadata browser packet using the stable DOM contract,
   proving the registered save advances only metadata version while the Fly canvas
   and document authoring remain absent.
3. Retain publish/rollback cache packets correlating receipts, outbox events, handler
   receipts, generation rotation and storefront/artifact misses and refills.
4. Complete compile, browser, workflow and rollout evidence before promoting FFA/FBA
   status beyond source-connected.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
cargo test -p rustok-pages-admin stale_metadata_revision_short_circuits_before_patch_transport
cargo test -p rustok-pages-admin metadata_save_is_document_free_and_preserves_dirty_fly_state
cargo test -p rustok-pages-admin published_page_admits_registered_metadata_surface
cargo test -p rustok-pages-admin non_published_or_missing_page_hides_registered_metadata_surface
cargo check -p rustok-page-builder-admin --all-targets
cargo check -p rustok-pages-admin --all-targets
```
