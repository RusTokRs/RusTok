# Implementation Plan for `rustok-pages`

## Current state

`rustok-pages` owns pages, bodies, blocks, menus, visibility, and the page
publish pipeline. Its admin and storefront packages use the module-owned
core/transport/Leptos split. Storefront keeps both the native server-function
and GraphQL selected paths; neither UI package has a legacy `api.rs` facade.

Pages is the reference consumer of the `grapesjs` Page Builder capability.
Its manifest fixes the consumer version, capability set, typed disabled states,
and four fallback profiles. `pages-wave0-dry-run-evidence.json` is explicitly
synthetic and the Wave 1 packet is a readiness draft, so neither is production
rollout evidence.

The admin package now mounts the selected page through
`PageBuilderAdminHostContext`. Its `PagesBuilderFacade` accepts the canonical
Page Builder publish envelope, fetches current page metadata, rejects stale
Page body `updated_at` revisions, writes through the existing Pages transport
facade, re-reads the persisted Page body, and returns that body revision to Fly.
The old CRUD/JSON form remains mounted as the metadata owner and fallback.

Canonical Fly editing uses `pages[].component`. For compatibility with the
existing Pages tree/preview helpers, the project codec keeps
`frames[0].component` as a synchronized snapshot. Legacy frame content is
copied into the canonical component on first load; canonical content wins and
refreshes the frame snapshot on subsequent saves. Existing blocks remain
attached and are not silently converted or removed.

## FFA/FBA status

- FFA status: `in_progress` — the Pages admin package has a real Fly consumer
  facade and iframe authoring composition, while storefront inline editing and
  module contribution factories remain open.
- FBA status: `boundary_ready` — Pages has the reference consumer metadata,
  optimistic revision protection and static fallback coverage, but no observed
  tenant control-plane evidence.
- Structural shape: `core_transport_ui`
- Evidence: `scripts/verify/verify-pages-ui-boundary.mjs`,
  `scripts/verify/verify-fly-admin-browser-runtime.mjs`,
  `crates/rustok-page-builder/contracts/evidence/pages-wave0-dry-run-evidence.json`,
  and `crates/rustok-page-builder/contracts/evidence/pages-wave1-readiness-draft.json`.

## Completed implementation slice — 2026-07-13

- The selected Pages document is loaded into `AdminCanvasController`.
- Pages persists only through its module-owned `transport` facade.
- `PageBuilderCapabilityRequest::Publish` is the only accepted editor write.
- Save compares the request revision with the latest Page body `updated_at`.
- After mutation, Pages re-reads the document and acknowledges the persisted
  body `updated_at`, not the separate Page entity timestamp.
- A stale editor receives stable code `REVISION_CONFLICT` and does not overwrite
  newer page data. The browser problem transport now preserves stable codes for revision,
  project-hash and draft-generation conflicts.
- Chromium coverage proves that HTTP 409 leaves the adapter revision, project hash and draft
  session unchanged; only a successful retry after an explicit host refresh advances them.
- The Page Builder iframe reports viewport, component geometry, pointer, hover
  and focus events through a source/origin/protocol/instance/sequence validated
  bridge.
- Fly UI receives selection and overlay state; the parent draws hover and
  selection outlines over the isolated iframe.
- The old Pages JSON editor remains available as the migration/fallback path.
- The Fly and JSON editors do not yet share a live refetch event; after a Fly
  save, the fallback form may remain visually stale until its own resource
  reloads, although persisted data and the next load are correct.
- English and Russian UI messages cover builder loading and bridge state.

## Open results

1. Implement real component mutation UX: block palette, drag/drop candidate
   zones, insert/move/remove controls, traits, styles and resize handles. Done
   when interactions produce only Fly commands, preserve undo/redo invariants,
   and pass browser interaction tests.
2. Add real GrapesJS browser captures and cross-editor round-trip evidence.
   Done when projects captured through `getProjectData()` load in Fly, survive
   edits, reload through GrapesJS and preserve unknown/plugin data.
3. Add a shared save/refetch event so the fallback metadata/JSON form refreshes
   after a successful Fly write without coupling Fly to Pages form state.
4. Run Wave 0 against an internal tenant and replace the synthetic evidence
   packet with observed before/after flag and health snapshots, smoke results,
   metrics, traces, and owner decision. Done when the packet is accepted by
   the evidence and correlation gates without placeholder values.
   Dependency: a runnable Page Builder control plane. Verification:
   `node crates/rustok-page-builder/scripts/verify/verify-page-builder-wave-evidence-packet.mjs`
   and `node crates/rustok-page-builder/scripts/verify/verify-page-builder-correlation-evidence.mjs`.
5. Promote the reference consumer through a real Wave 1 only after the Wave 0
   result and provider persistence/rendering paths are verified. Done when an
   approved tenant packet proves `preview -> properties -> publish(dry)`, all
   fallback profiles, rollback execution, and the correlation
   `builder write -> pages publish -> storefront read`.
6. Decide and execute the legacy-frame/legacy-block exit policy. Done when the
   owner has recorded migration preconditions and the compatibility snapshot can
   be removed without breaking old Pages tooling or deleting existing blocks.
7. Implement Pages storefront renderers and optional authenticated real-DOM
   inline editing without leaking editor dependencies into anonymous bundles.

## Verification

- `node scripts/verify/verify-pages-ui-boundary.mjs`
- `node scripts/verify/verify-fly-admin-browser-runtime.mjs`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:pages:legacy-bridge`
- `npm run verify:page-builder:wave1-readiness-draft`
- targeted Rust tests for `rustok-pages-admin`, `rustok-page-builder-admin`,
  `fly`, `fly-ui` and `fly-leptos`
- Browser interaction tests for iframe handshake, source/origin/instance rejection,
  sequence replay protection, selection, geometry, teardown and stale-save retry behavior

## Boundaries

- Pages owns page/menu lifecycle, metadata, visibility, published reads,
  optimistic body revisions and migration safety for its existing blocks.
- Page Builder admin owns editor behavior and the canonical facade envelope, but
  does not choose the Pages transport or directly persist a page.
- Fly owns the canonical project model, codec, commands and revision hash. The
  legacy frame component is a temporary compatibility snapshot, not a second
  editor authority.
- Page Builder backend owns capability policy, validation/sanitization seams,
  feature flags and control-plane rollout mechanics.
- Hosts compose module packages and provide route/auth/tenant context; they do
  not take ownership of page policy, Fly state or persistence semantics.
