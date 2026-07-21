# Implementation Plan for `rustok-pages`

## Policy: no legacy

Pages is under active development and carries **no legacy compatibility policy**.
Old parallel editors, data mirrors, block APIs and migration shims are deleted
instead of being kept as fallback paths.

Forbidden architecture:

- a JSON/CRUD editor beside Fly;
- `admin/src/ui` as a second admin application;
- `pages[].component` mirrored into `frames[0].component`;
- importing historical frame trees into the current document;
- `PageBlock` in the current admin/storefront contract;
- writes that preserve obsolete payloads only for compatibility;
- host-owned Pages logic or direct UI access to raw GraphQL adapters.

The only authoring source of truth is the current Fly document at
`pages[].component` plus Pages-owned metadata and lifecycle state.

## Mission

`rustok-pages` owns page identity, localized metadata, slugs, channels, menus,
draft/published lifecycle, immutable published landing artifacts and storefront
reads. Page Builder/Fly owns visual document authoring and rendering primitives,
but never owns Pages persistence, routing or tenant policy.

The target is:

- one builder-first admin workspace;
- one current document contract;
- deterministic publish artifacts;
- FFA-owned admin/storefront composition;
- FBA-owned policy, persistence/rendering ports and rollout controls;
- no compatibility branches or shadow state.

## Current implementation

### Admin FFA

- [x] `PagesAdmin` is the sole public admin entrypoint.
- [x] The parallel 1,200+ line JSON/CRUD UI and its module files are deleted.
- [x] The workspace lists pages, creates a current Fly document, selects a page,
  mounts Fly, publishes/unpublishes and deletes Pages documents.
- [x] New pages start with `pages[].component`; no frame mirror is generated.
- [x] `PagesBuilderFacade` accepts only the canonical publish request, reloads
  current metadata, verifies the body revision, persists through Pages transport,
  rereads the body and acknowledges the persisted revision.
- [x] Fly saves refresh the current Pages list/workspace resource.
- [x] Admin GraphQL no longer requests or writes `blocks`.
- [x] `PageBlock`, compatibility preview/tree helpers and deleted UI dependencies
  are removed from the admin package.
- [x] Pages admin contributions expose current Fly builtin landing blocks through
  provider/capability policy.

### Storefront FFA

- [x] Published `grapesjs` documents render through the Page Builder storefront
  package.
- [x] Published static landing artifacts have a dedicated rendering path.
- [ ] Storefront model/query code still contains historical block fallback fields;
  these must be deleted with the backend block subsystem.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Anonymous-bundle exclusion evidence is not complete.

### Backend/FBA

- [x] Pages has tenant-scoped capability metadata, typed errors, endpoint adapter
  seams, optimistic body revisions and rollout evidence contracts.
- [x] The repository contains a deterministic landing build/publish pipeline with
  renderer identity, SHA-256 build/artifact integrity and immutable landing
  artifact records.
- [x] Published landing artifacts and scenario baselines have dedicated entities
  and services.
- [ ] The old `page_blocks` entity/service/GraphQL/migration surface still exists
  and must be removed.
- [ ] Metadata-only patch semantics are missing; body writes are still coupled to
  metadata updates.
- [ ] Observed tenant Wave 0/Wave 1 evidence is not complete.

## FFA/FBA status

- **FFA:** `in_progress` — builder-only admin is established; metadata properties,
  inline storefront editing and generated multi-module contribution registries
  remain open.
- **FBA:** `in_progress` — deterministic landing artifact publication exists;
  obsolete block persistence, complete metadata patching, rollout evidence and
  operational repair paths remain open.
- **Structural shape:** `core_transport_ui` with one current admin composition.

## Ownership boundaries

- **Pages domain/backend:** identity, translations, slugs, channels, templates,
  menus, revision state, publish transaction, artifact selection, redirects,
  deletion and audit.
- **Pages admin FFA:** list/create/select workspace, metadata contribution UI,
  Pages facade, permissions and resource refresh.
- **Pages storefront FFA:** published route reads, renderer composition, cache
  integration and optional authenticated edit mode.
- **Page Builder admin:** editor behaviour and canonical capability envelope.
- **Fly:** current document model, commands, history, registries, validation,
  deterministic rendering and document hash.
- **Page Builder backend FBA:** capability policy, validation/sanitization ports,
  health, feature flags and rollout mechanics.
- **Hosts:** route, locale, auth and tenant context only.

## Current document contract

```text
Page metadata
  + current Fly document (pages[].component)
  + optimistic body revision
  -> validation/readiness
  -> deterministic renderer
  -> immutable landing artifact
  -> atomic published artifact pointer
  -> storefront read/cache
```

Invariants:

1. `pages[].component` is the only component-tree authority.
2. Unknown current provider/plugin fields are preserved by the Fly codec.
3. Missing providers fail visibly; they never trigger silent deletion.
4. Draft saves do not mutate the selected published artifact.
5. Publish validates and renders a deterministic artifact before switching the
   published pointer.
6. Artifact identity includes source, renderer release, registry and render
   policy hashes.
7. Dynamic widgets store versioned configuration, not resolved privileged data.
8. Anonymous storefront bundles contain no editor code.
9. There is no fallback to obsolete editors, frame mirrors or block tables.

## Completed slice — 2026-07-21

- Deleted `crates/rustok-pages/admin/src/ui` and removed it from the crate root.
- Replaced the parallel editor with one builder-first Pages workspace.
- Added current page creation, navigation, publish/unpublish and delete actions.
- Removed admin `PageBlock` data, block GraphQL fields and obsolete UI dependencies.
- Reduced admin core to current document/domain helpers.
- Changed canonicalization to create/read only `pages[].component`; historical
  frame trees are neither imported nor synchronized.
- Replaced Pages boundary checks and fixtures with no-legacy guardrails.

## Implementation order

### P0 — delete the backend block subsystem

- [ ] Remove `entities/page_block.rs`, `BlockService`, block controllers and
  GraphQL block mutations/fields.
- [ ] Remove block relations from Page entities and service projections.
- [ ] Rewrite the initial Pages migration so fresh installations never create
  `page_blocks`; remove follow-up ordering constraints for that table.
- [ ] Remove storefront block models, summaries and fallback rendering.
- [ ] Remove old Next admin page-builder/block code and obsolete docs/tests.
- [ ] Add repository-wide guardrails rejecting `PageBlock`, `BlockService` and
  `page_blocks` outside deleted migration history (there should be none after the
  migration rewrite).

**Gate:** `rg "PageBlock|BlockService|page_blocks"` returns no production source.

### P0 — separate metadata and document writes

- [ ] Add a typed metadata patch command for title, slug, locale, channels,
  template and SEO fields that never accepts body/project data.
- [ ] Add a typed document save command carrying only page id, body revision and
  Fly project.
- [ ] Make metadata and document revisions explicit and independently conflict
  checked.
- [ ] Move metadata editing into Pages-owned Page Builder property contributions.
- [ ] Add slug/locale uniqueness errors and route preview before commit.

**Gate:** editing metadata cannot overwrite a dirty/current Fly document, and a
Fly save cannot revert metadata.

### P0 — authoritative publish transaction

- [x] Deterministic Fly landing artifact build and integrity identity.
- [x] Pages landing artifact persistence entities/services.
- [ ] Make publish an idempotent transaction: validate -> sanitize -> build ->
  persist artifact -> atomically select published artifact -> outbox/cache events.
- [ ] Add rollback to a prior immutable artifact.
- [ ] Correlate editor save, document revision, publish operation, artifact and
  storefront read.
- [ ] Add repair/rebuild commands for missing/corrupt artifacts and route indexes.

### P1 — complete Pages/Page Builder FFA

- [ ] Add typed Pages metadata property editors.
- [ ] Add Media asset contributions and current asset picker contracts.
- [ ] Add rich-text only through the dedicated opaque payload/editor seam.
- [ ] Generate admin/storefront contribution registries from module metadata.
- [ ] Filter contributions by tenant, permission, capability, provider health and
  surface.
- [ ] Add complete keyboard, accessibility, degraded-state and permission tests.

### P1 — storefront and routing

- [ ] Render only the selected immutable published artifact.
- [ ] Add locale fallback, canonical URLs, redirect records and route collision
  policy.
- [ ] Integrate menus, SEO and channel visibility with deterministic cache keys.
- [ ] Add authenticated real-DOM inline editing behind explicit permissions and
  flags.
- [ ] Prove anonymous SSR/CSR/hydrate bundles exclude Fly authoring code.
- [ ] Prove admin preview, published artifact and inline-edit visual parity.

### P2 — operations and rollout

- [ ] Audit draft save, metadata patch, publish, unpublish, rollback and delete.
- [ ] Metrics: save/publish latency, conflict rate, validation/sanitizer rejection,
  renderer failure, artifact integrity failure, missing provider and cache hit.
- [ ] Run observed internal-tenant Wave 0.
- [ ] Run Wave 1 after P0 gates pass; no synthetic packet may count as rollout.
- [ ] Prove rollback for provider, sanitizer, renderer and contribution failures.

## Verification

Fast checks:

- `node scripts/verify/verify-pages-ui-boundary.mjs`
- `node --test scripts/verify/verify-pages-ui-boundary.test.mjs`
- `node scripts/verify/verify-fly-admin-browser-runtime.mjs`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:fba:baseline`
- `npm run verify:i18n:ui`
- `npm run verify:i18n:contract`

Rust/WASM checks:

- `cargo test -p rustok-pages-admin`
- `cargo test -p rustok-pages-storefront`
- `cargo test -p rustok-pages`
- `cargo test -p rustok-page-builder`
- `cargo test -p rustok-page-builder-admin`
- `cargo test -p rustok-page-builder-storefront`
- `cargo test -p fly`
- `cargo test -p fly-ui`
- `cargo test -p fly-leptos`
- `cargo xtask module validate pages`
- `cargo xtask module validate page_builder`

No-legacy checks:

- admin `ui/`, `api.rs` and compatibility shims do not exist;
- no JSON project textarea exists outside tests/fixtures;
- no frame copy/synchronization helper exists;
- no current admin/storefront GraphQL field named `blocks` exists;
- after P0 cleanup, no `PageBlock`, `BlockService` or `page_blocks` production
  symbol exists.

## Update rules

- This plan is updated in every Pages implementation slice.
- Checkboxes require merged source; gates require reproducible executed evidence.
- Contract changes require matching guardrails and tests.
- New dependencies require dependency records.
- Do not reintroduce legacy compatibility, shadow editors or duplicate document
  authorities.
