# Pages / Page Builder Parity Continuation Plan

Date: 2026-08-03
Status: source-cutover-complete / published-surface-regression-ready / cache-correlation-source-ready / execution-evidence-pending
Scope: `rustok-pages` admin FFA and `rustok-page-builder` consumer-property and publication/cache boundaries

## Audit basis

This plan reconciles the current source with:

- `crates/rustok-pages/docs/implementation-plan.md`;
- `crates/rustok-page-builder/docs/implementation-plan.md`;
- `crates/rustok-page-builder/contracts/page-builder-consumer-properties.json`;
- the Pages admin composition, rollback control and metadata owner port;
- reviewed publish and immutable rollback owner transactions;
- the Pages cache invalidation owner and generation-aware storefront/artifact readers;
- the Page Builder consumer-property runtime and Leptos panel.

The audit remains source-only. No verifier, formatter, Cargo command, browser
scenario, database scenario, storefront request, artifact HTTP request, workflow or
CI run is claimed by this update.

## Corrected parity snapshot

### Rollback control: source-connected

The Pages admin mounts `PagesRollbackControl`. It loads the selected page, renders
only for published pages, requires prepare/confirm, delegates to the typed rollback
transport, consumes the typed result version and refreshes the workspace after a
successful receipt.

Accepted rollback, outbox, cache-generation and storefront refill execution packets
remain open.

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

The standalone published surface uses one closed admission state:

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
port, whose independent revision and dirty-Fly isolation packet is linked by the
evidence contract.

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs`;
- the focused tests in `crates/rustok-pages/admin/src/standalone_metadata.rs`.

Browser execution remains pending. The DOM markers are selectors for a retained
browser packet; they are not a claim that the browser scenario ran.

### Publish/rollback cache correlation source packet: ready, unvalidated

Reviewed publish and immutable rollback each write `NodeUpdated` and `NodePublished`
through the owner transaction, then insert their durable operation receipt before the
same transaction commits. Neither service calls cache infrastructure inline.

The Pages cache handler maps the root `NodePublished` envelope to a request carrying
the exact event and correlation identities. Published invalidation owns the route,
page and artifact scopes, and the runtime validates an event/correlation-bound receipt
with a positive generation for every requested scope before the handler acknowledges
the event.

The focused integration regression uses one shared implementation of
`PageCacheInvalidationPort` and `PagesCacheReadPort`. It pre-fills old storefront and
artifact keys, dispatches one `NodePublished` envelope through the real
`PageCacheInvalidationEventHandler`, and records the exact request, receipt and new
generation snapshot.

The regression then proves:

- route, page and artifact generations each advance once;
- the request and handler receipt retain the exact envelope event and correlation ids;
- the composite storefront key changes when the generation snapshot changes;
- the artifact key changes when the artifact generation changes;
- new current-generation keys miss before refill and hit after refill;
- old generation keys remain physically present but unreachable through current key
  construction.

The production storefront reader still orders authorization, generation snapshot,
cache lookup, owner source/artifact read and cache fill. Artifact HTTP delivery keeps
the equivalent authorization, generation, lookup, verified owner artifact and fill
order. Reader failures continue to fail open to owner source reads.

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-publish-rollback-cache-correlation-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-cache-correlation.mjs`;
- `crates/rustok-pages/tests/publish_rollback_cache_correlation.rs`.

Execution remains pending. No database transaction, durable outbox relay, real cache,
storefront request or artifact HTTP request was executed by this update.

### Canonical plans: actualized to source parity

The Pages and Page Builder canonical implementation plans mark the typed rollback
control, registered metadata contribution, draft/published panel composition and
legacy editor removal as source-complete. Their executed metadata, cache, browser,
workflow and rollout packets remain open.

## Parity matrix

| Capability | Pages owner | Page Builder owner | Source state | Evidence state |
| --- | --- | --- | --- | --- |
| Metadata schema and values | Pages | Contract/runtime validation | Connected | Source regression ready; execution pending |
| Metadata optimistic revision | Pages | Typed save envelope | Connected | Conflict regression ready; execution pending |
| Dirty Fly isolation during metadata save | Pages metadata port | Fly/Page Builder controller | Source-guarded | Isolation regression ready; execution pending |
| Draft metadata panel inside Fly | Pages runtime/port | Leptos consumer panel | Connected | Pending browser execution |
| Published metadata without Fly canvas | Pages shell composition | Standalone consumer panel | Admission and DOM source-guarded | Browser execution pending |
| Publish/rollback outbox to generation rotation | Pages lifecycle/cache owners | No cache ownership | Correlation regression ready | Execution pending |
| Storefront/artifact generation miss and refill | Pages readers | Immutable artifact provider | Source regression ready | Real reader packet pending |
| Legacy metadata form | None | None | Removed | Not applicable |
| Immutable artifact rollback action | Pages | No lifecycle ownership | Connected | Source correlation ready; database packet pending |
| Fly document mutation | Pages builder facade | Fly/Page Builder | Draft-only | Pending observed packet |
| Published Fly authoring | Not allowed | Not mounted | Correctly blocked | Pending bundle/runtime proof |

## Changes in this slice

1. Add one integration regression over the public Pages cache invalidation and read
   contracts.
2. Pre-fill generation-bound storefront and artifact values before a published event.
3. Dispatch `NodePublished` through the real Pages cache handler and retain exact
   event/correlation request and receipt identities.
4. Prove all three published generations advance and produce new current keys.
5. Prove current keys miss, refill and then hit while old generation values remain
   physically present.
6. Add a machine-readable source evidence contract and focused cross-file verifier.
7. Source-lock reviewed publish and rollback as transactional `NodePublished`
   producers with durable receipts before commit and no inline cache calls.
8. Actualize this parity plan while keeping every execution claim open.

## Boundaries

This slice does not:

- change Pages publish, rollback, cache, storefront or artifact production behavior;
- change database entities, migrations, DTOs, GraphQL, HTTP or browser transport;
- add inline cache invalidation to publish or rollback;
- delete old cache values, scan namespaces or introduce per-page generation state;
- change Page Builder capability authorization, rollout policy or contribution
  assembly semantics;
- run a database, outbox relay, cache backend, storefront or artifact HTTP scenario;
- claim executed evidence or test results.

## Next cursor

1. Run and retain the focused metadata conflict and dirty-Fly isolation packets.
2. Run and retain the published metadata browser packet using the stable DOM contract,
   proving the registered save advances only metadata version while the Fly canvas
   and document authoring remain absent.
3. Run the cache correlation source packet, then retain a database/outbox/cache packet
   tying real publish and rollback operation receipts to durable `NodePublished`
   envelopes, handler receipts, generation changes and real storefront/artifact
   miss/refill observations.
4. Complete compile, browser, workflow and rollout evidence before promoting FFA/FBA
   status beyond source-connected.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs
node crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-published-metadata-surface.mjs
node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-cache-correlation.mjs
cargo test -p rustok-pages-admin stale_metadata_revision_short_circuits_before_patch_transport
cargo test -p rustok-pages-admin metadata_save_is_document_free_and_preserves_dirty_fly_state
cargo test -p rustok-pages-admin published_page_admits_registered_metadata_surface
cargo test -p rustok-pages-admin non_published_or_missing_page_hides_registered_metadata_surface
cargo test -p rustok-pages --test publish_rollback_cache_correlation -- --nocapture
cargo check -p rustok-page-builder-admin --all-targets
cargo check -p rustok-pages-admin --all-targets
cargo check -p rustok-pages --all-targets
```
