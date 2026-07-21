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
menus, draft/published lifecycle, immutable landing artifacts, publish receipts,
routes and storefront reads. Fly/Page Builder owns visual document primitives and
capability contracts, not Pages persistence or tenant policy.

## Current implementation

### Domain and persistence

- [x] Pages has independent entities for pages, translations, bodies, channel
  visibility, scenario baselines, immutable landing artifacts and publish receipts.
- [x] `PageBlock`, `BlockService`, block DTOs, relations, GraphQL/REST/OpenAPI
  surfaces and storefront block models are deleted.
- [x] The initial development migration never creates `page_blocks`; no drop or
  compatibility migration is retained.
- [x] `PageService` is split into focused current-only modules instead of one
  block-aware monolith.
- [x] New/current documents use only `pages[].component`.
- [x] Unknown current provider/plugin fields are preserved by the Fly codec.
- [x] Page writes use optimistic page versions and body revisions.
- [x] Builder feature flags and scenario-baseline gates fail with typed errors.
- [x] Static landing records persist Page Builder materialization hash, identity
  and runtime snapshot evidence without storing raw runtime context.
- [x] `page_publish_operations` stores one durable result per
  `(tenant_id, page_id, idempotency_key)` with request, sanitization and artifact
  set hashes; it never stores the reviewed runtime context.

### Admin FFA

- [x] Pages owns the Page Builder consumer facade and transport selection.
- [x] Fly saves reload current page metadata and reject stale body revisions.
- [x] Pages contributes current Fly landing blocks through provider/capability
  policy.
- [ ] Public admin publish transport still needs atomic reviewed-runtime cutover.
- [ ] Metadata editing still needs a typed metadata-only property contribution.

### Storefront FFA

- [x] Published `grapesjs` documents render through Page Builder storefront.
- [x] Static published landing artifacts have a dedicated sandboxed path.
- [x] Storefront GraphQL/native adapters no longer query or synthesize blocks.
- [x] Bound static artifacts are integrity-checked in the same transaction before
  storefront HTML is returned. New records verify the complete Page Builder
  materialization envelope; legacy records are accepted only with all evidence
  columns `NULL` and a valid Fly artifact.
- [ ] Storefront should read only the selected immutable published artifact.
- [ ] Authenticated real-DOM inline editing is not implemented.
- [ ] Anonymous bundle exclusion evidence is not complete.

### Page Builder/FBA

- [x] Capability registry, permissions, typed errors, fallback profiles and
  endpoint adapter seams exist.
- [x] Deterministic Fly landing rendering and SHA-256 artifact identity exist.
- [x] Pages persists immutable landing artifact records and bindings.
- [x] Pages persists runtime materialization identity/snapshots with a composite
  uniqueness key that includes `materialization_hash`; partial evidence is
  rejected and raw runtime context is forbidden.
- [x] The provider exposes an explicit reviewed publish-runtime contract. Pages
  verifies its scenario/context hash against materialization identity.
- [x] Page Builder exposes authoritative static publish sanitization through
  `sanitize_static_landing_project`, including stable ids, structural validation
  and secure public-resource policy before materialization.
- [x] `PageService::publish_reviewed` is one idempotent transaction covering page
  and body locks, feature/baseline gates, sanitization, materialization, immutable
  persistence, binding, page transition, transactional outbox events and receipt.
- [x] A replay with the same request hash returns the stored receipt without
  rebuilding artifacts or emitting duplicate events; key reuse with another
  request fails with a typed conflict.
- [ ] GraphQL, HTTP and admin publish transports must be cut over from the legacy
  default-runtime path to `PublishPageInput`.
- [ ] Dedicated cache-consumer invalidation evidence remains open even though the
  publish transaction emits its durable `NodePublished` outbox signal.
- [ ] Observed tenant Wave 0/Wave 1 evidence remains open.

## FFA/FBA status

- **FFA:** `in_progress` — current-only runtime/storefront boundaries are ready;
  atomic publish transport cutover, typed metadata properties and inline edit mode
  remain open.
- **FBA:** `in_progress` — reviewed runtime, authoritative sanitizer, immutable
  materialization evidence and idempotent atomic Pages service are integrated;
  public transport cutover, rollback, cache-consumer proof and observed rollout
  evidence remain open.
- **Structural shape:** `core_transport_ui` with one current document authority.

## Ownership boundaries

- **Pages domain/backend:** identity, translations, slugs, channels, templates,
  menus, revisions, publish transaction, receipts, artifact selection, redirects,
  deletion and audit.
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
Page metadata version
  + exact localized body revisions
  + idempotency key
  + reviewed runtime scenario/context hash
  -> page/body locks
  -> feature and promoted-scenario gates
  -> authoritative sanitization
  -> canonical runtime materialization
  -> deterministic renderer
  -> immutable landing artifacts + materialization evidence
  -> published artifact bindings
  -> published page state
  -> transactional NodeUpdated/NodePublished outbox
  -> durable publish receipt
  -> verified route/cache/storefront read
```

Invariants:

1. `pages[].component` is the sole component-tree authority.
2. Metadata and document writes never overwrite one another implicitly.
3. Draft saves do not mutate the selected published artifact.
4. Publish rejects stale metadata or any stale localized body revision.
5. Artifact identity includes source, renderer release, registry and policy hashes.
6. Materialization evidence includes context hash, scenario identity and runtime
   snapshot hash; raw context is never stored.
7. Reviewed runtime is valid only when SHA-256 binds format, explicit scenario and
   transient context, and promoted baseline evidence matches that scenario/context.
8. Authoritative sanitization happens before runtime materialization and is bound
   into the operation through `sanitized_set_hash`.
9. A committed idempotency key is immutable: exact replay returns its receipt;
   different input fails closed.
10. Page state, artifact bindings, outbox events and publish receipt commit or roll
    back together.
11. Missing providers fail visibly and never cause silent deletion.
12. Dynamic widgets persist versioned configuration, not privileged snapshots.
13. Anonymous storefront bundles contain no editor code.
14. No block or shadow-editor fallback exists.

## Completed slice — 2026-07-21

- Removed the block entity/DTO/service/GraphQL/REST/OpenAPI contract and all
  storefront block fallback rendering.
- Split `PageService` into focused current-only modules and retained
  `pages[].component` as the sole visual authority.
- Added immutable static landing artifacts, materialization evidence and strict
  storefront verification with fail-closed partial evidence.
- Added `PageBuilderReviewedPublishRuntime`, binding format, explicit scenario and
  transient context through a SHA-256 review hash.
- Added `sanitize_static_landing_project`, which produces a verified deterministic
  project and SHA-256 sanitization identity before materialization.
- Added `PublishPageInput` with page version, exact locale/body revisions,
  idempotency key and reviewed runtime.
- Added `page_publish_operations` and its unique tenant/page/key receipt boundary.
- Replaced the provisional reviewed domain path with one atomic
  `PageService::publish_reviewed` service. It locks page and bodies, validates
  promoted runtime scenario/context, sanitizes, materializes, persists and binds
  immutable artifacts, updates page state, writes outbox events and inserts the
  receipt in one transaction.
- Added typed errors for review, sanitization, materialization mismatch,
  idempotency collision and receipt integrity.
- Expanded the machine-readable contract and source guardrail. Raw runtime context
  remains forbidden in both artifacts and publish receipts.

## Next implementation order

### P0 — separate metadata and document writes

- [ ] Finish Pages-owned typed metadata property contributions.
- [ ] Track metadata and document revisions independently in every transport.
- [ ] Add conflict tests proving metadata saves cannot replace a dirty/current Fly
  document and Fly saves cannot revert metadata.

### P0 — atomic artifact publication

- [x] Deterministic renderer and artifact identity.
- [x] Immutable artifact persistence and body bindings.
- [x] Runtime materialization identity/snapshot persistence and storefront
  verification.
- [x] Explicit reviewed publish-runtime/scenario contract.
- [x] Authoritative sanitizer before materialization.
- [x] Idempotent atomic reviewed service: lock -> validate -> sanitize -> materialize
  -> compile -> persist -> bind -> state -> outbox -> receipt.
- [ ] Cut GraphQL, HTTP and admin transports over to `PublishPageInput`; remove
  builder publication through the default runtime and disable create-and-publish.
- [ ] Add rollback to a previous immutable artifact.
- [ ] Correlate receipt, editor save, page/body revisions, runtime review,
  materialization, artifact and storefront read in operational telemetry.
- [ ] Add integrity audit and repair/rebuild commands.
- [ ] Prove the `NodePublished` outbox consumer invalidates every affected cache key.

### P1 — complete Page Builder authoring

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

- [ ] Audit metadata save, document save, publish, replay, unpublish, rollback and
  delete.
- [ ] Metrics for save/publish latency, conflicts, sanitizer rejection, renderer
  failure, artifact/receipt integrity, missing providers and cache hit rate.
- [ ] Run observed internal-tenant Wave 0.
- [ ] Run Wave 1 only after transport publication/rollback gates pass.
- [ ] Prove rollback for provider, sanitizer, renderer and contribution failures.

## Verification

- `cargo test -p rustok-pages --lib`
- `cargo clippy -p rustok-pages --lib -- -D warnings`
- `cargo test -p rustok-pages-admin --lib`
- `cargo check -p rustok-pages-storefront --lib`
- `cargo clippy -p rustok-pages-storefront --lib -- -D warnings`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-preview-runtime-contract.mjs`
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-publish-runtime-review.mjs`
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
