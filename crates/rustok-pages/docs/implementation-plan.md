# Implementation Plan for `rustok-pages`

## Policy: current code only

Pages is under active development. It does not keep compatibility editors,
component mirrors, block tables or migration shims.

Forbidden:

- a JSON/CRUD editor beside Fly;
- the deleted Next/GrapesJS page-builder route;
- `frames[0].component` as a component-tree mirror;
- `PageBlock`, `BlockService`, `page_blocks` or block mutations;
- storefront block fallback rendering;
- UI access to raw transport adapters;
- host-owned Pages persistence or document policy.

The visual document authority is `pages[].component` stored in the Pages body.

## Mission

`rustok-pages` owns page identity, localized metadata and bodies, slugs, channels,
menus, draft/published lifecycle, immutable landing artifacts, routes and
storefront reads. Fly/Page Builder owns visual document primitives and capability
contracts, not Pages persistence or tenant policy.

## Current implementation

### Domain and persistence

- [x] Pages has independent entities for pages, translations, bodies, channel
  visibility, scenario baselines and immutable landing artifacts.
- [x] `PageBlock`, `BlockService`, block DTOs, relations, GraphQL/REST/OpenAPI
  surfaces and storefront block models are deleted.
- [x] The initial development migration never creates `page_blocks`; no drop or
  compatibility migration is retained.
- [x] `PageService` is split into `create`, `read`, `update`, `persistence` and
  `helpers` modules instead of one block-aware monolith.
- [x] New/current documents use only `pages[].component`.
- [x] Unknown current provider/plugin fields are preserved by the Fly codec.
- [x] Page writes use optimistic page versions and body revisions.
- [x] Builder feature flags and scenario-baseline gates fail with typed errors.

### Admin FFA

- [x] Pages owns the Page Builder consumer facade and transport selection.
- [x] Fly saves reload current page metadata and reject stale body revisions.
- [x] Pages contributes current Fly landing blocks through provider/capability
  policy.
- [ ] The separate builder-first admin workspace is delivered by the companion
  current-only admin change.
- [ ] Metadata editing still needs a typed metadata-only patch/property contract.

### Storefront FFA

- [x] Published `grapesjs` documents render through Page Builder storefront.
- [x] Static published landing artifacts have a dedicated sandboxed path.
- [x] Storefront GraphQL/native adapters no longer query or synthesize blocks.
- [ ] Storefront should read only the selected immutable published artifact.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Anonymous bundle exclusion evidence is not complete.

### Page Builder/FBA

- [x] Capability registry, permissions, typed errors, fallback profiles and
  endpoint adapter seams exist.
- [x] Deterministic Fly landing rendering and SHA-256 artifact identity exist.
- [x] Pages persists immutable landing artifact records and bindings.
- [ ] Publish must become one idempotent atomic workflow from validation through
  artifact binding and outbox/cache invalidation.
- [ ] Authoritative sanitization is not complete for every publish path.
- [ ] Observed tenant Wave 0/Wave 1 evidence remains open.

## FFA/FBA status

- **FFA:** `in_progress` — current-only runtime/storefront boundaries are ready;
  typed metadata properties and inline edit mode remain open.
- **FBA:** `in_progress` — deterministic artifact primitives exist; atomic
  publication, rollback, sanitization and observed rollout evidence remain open.
- **Structural shape:** `core_transport_ui` with one current document authority.

## Ownership boundaries

- **Pages domain/backend:** identity, translations, slugs, channels, templates,
  menus, revisions, publish transaction, artifact selection, redirects, deletion
  and audit.
- **Pages admin FFA:** list/create/select workspace, metadata property
  contributions, Pages persistence facade and permissions.
- **Pages storefront FFA:** published reads, routing, renderer composition, cache
  integration and optional authenticated edit mode.
- **Page Builder admin:** editor behaviour and canonical capability envelope.
- **Fly:** current project model, commands, history, registries, validation,
  deterministic rendering and document hash.
- **Page Builder backend FBA:** capability policy, validation/sanitization ports,
  health, feature flags and rollout mechanics.
- **Hosts:** route, locale, auth and tenant context only.

## Current document/publication model

```text
Page metadata revision
  + Fly document/body revision
  -> validation and provider readiness
  -> authoritative sanitization
  -> deterministic renderer
  -> immutable landing artifact
  -> atomic published artifact pointer
  -> route/cache/storefront read
```

Invariants:

1. `pages[].component` is the sole component-tree authority.
2. Metadata and document writes never overwrite one another implicitly.
3. Draft saves do not mutate the selected published artifact.
4. Publish validates and builds before making output visible.
5. Artifact identity includes source, renderer release, registry and policy hashes.
6. Missing providers fail visibly and never cause silent deletion.
7. Dynamic widgets persist versioned configuration, not privileged snapshots.
8. Anonymous storefront bundles contain no editor code.
9. No block or shadow-editor fallback exists.

## Completed slice — 2026-07-21

- Removed the entire block entity/DTO/service/GraphQL/REST/OpenAPI contract.
- Removed block lifecycle from Page create/read/update/delete operations.
- Split `PageService` into focused current-only modules.
- Removed block fields from storefront GraphQL/native adapters, models and UI.
- Deleted the separate Next/GrapesJS editor route, component, API and navigation.
- Rewrote current round-trip tests around `pages[].component`.
- Added a no-block/no-shadow-editor source guardrail.
- Extended Fly/Page Builder CI to test, lint and format Pages domain/storefront.
- Rewrote the development schema so `page_blocks` is never created.

## Next implementation order

### P0 — separate metadata and document writes

- [ ] Add a typed metadata patch for title, slug, locale, channels, template and
  SEO fields that cannot carry body/project data.
- [ ] Add a typed document save command carrying page id, body revision and Fly
  project only.
- [ ] Track metadata and document revisions independently.
- [ ] Add conflict tests proving metadata saves cannot replace a dirty/current
  Fly document and Fly saves cannot revert metadata.
- [ ] Move metadata editing into Pages-owned Page Builder property contributions.

### P0 — atomic artifact publication

- [x] Deterministic renderer and artifact identity.
- [x] Immutable artifact persistence and body bindings.
- [ ] Make publish idempotent: validate -> sanitize -> compile -> persist -> bind
  -> switch published state -> outbox/cache invalidation.
- [ ] Add rollback to a previous immutable artifact.
- [ ] Correlate editor save, page revision, artifact and storefront read.
- [ ] Add integrity audit and repair/rebuild commands.

### P1 — complete Page Builder authoring

- [ ] Add typed Pages metadata property editors.
- [ ] Add Media asset contributions without transferring Media ownership.
- [ ] Integrate rich text only through the dedicated opaque payload/editor seam.
- [ ] Generate admin/storefront contribution registries from module metadata.
- [ ] Filter contributions by tenant, permission, capability, provider health and
  surface.
- [ ] Complete accessibility, keyboard and degraded-state coverage.

### P1 — storefront and routing

- [ ] Serve only the selected immutable published artifact.
- [ ] Add locale fallback, canonical URLs, redirects and route-collision policy.
- [ ] Integrate menus, SEO and channel visibility with deterministic cache keys.
- [ ] Implement authenticated real-DOM inline editing behind permissions/flags.
- [ ] Prove anonymous SSR/CSR/hydrate bundles exclude authoring code.
- [ ] Prove admin preview, published output and inline edit parity.

### P2 — operations and rollout

- [ ] Audit metadata save, document save, publish, unpublish, rollback and delete.
- [ ] Metrics for save/publish latency, conflicts, sanitizer rejection, renderer
  failure, artifact integrity, missing providers and cache hit rate.
- [ ] Run observed internal-tenant Wave 0.
- [ ] Run Wave 1 only after publication/rollback gates pass.
- [ ] Prove rollback for provider, sanitizer, renderer and contribution failures.

## Verification

- `cargo test -p rustok-pages --lib`
- `cargo clippy -p rustok-pages --lib -- -D warnings`
- `cargo test -p rustok-pages-admin --lib`
- `cargo check -p rustok-pages-storefront --lib`
- `cargo clippy -p rustok-pages-storefront --lib -- -D warnings`
- `node scripts/verify/verify-pages-current-only.mjs`
- `node scripts/verify/verify-pages-ui-boundary.mjs`
- `npm run verify:page-builder:consumer:pages`
- `npm run verify:page-builder:fba:baseline`
- `cargo xtask module validate pages`
- migration compatibility and full workspace CI

## Update rules

- Update this plan in every Pages implementation slice.
- Checkboxes require merged source; gates require reproducible executed evidence.
- Contract changes require matching guardrails/tests.
- New dependencies require dependency records.
- Never reintroduce block storage, shadow editors, frame mirrors or duplicate
  document authorities.
