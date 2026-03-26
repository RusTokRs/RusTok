# rustok-pages module implementation plan (`rustok-pages`)

## Scope and objective

This document captures the current implementation plan for `rustok-pages` in RusToK and
serves as the source of truth for rollout sequencing in `crates/rustok-pages`.

Primary objective: evolve `rustok-pages` in small, testable increments while preserving
compatibility with platform-level contracts.

## Target architecture

- `rustok-pages` remains focused on its bounded context and public crate API.
- Integrations with other modules go through stable interfaces in `rustok-core`
  (or dedicated integration crates where applicable).
- Behavior changes are introduced through additive, backward-compatible steps.
- Observability and operability requirements are part of delivery readiness.

## Delivery phases

### Phase 0 — Foundation (done)

- [x] Baseline crate/module structure is in place.
- [x] Base docs and registry presence are established.
- [x] Core compile-time integration with the workspace is available.

### Phase 1 — Contract hardening (done)

- [x] Freeze public API expectations for the current module surface.
- [x] Align error/validation conventions with platform guidance (RichError system).
- [x] Expand automated tests around core invariants and boundary behavior.

### Phase 2 — Domain expansion (planned)

- [ ] Implement prioritized domain capabilities for `rustok-pages`.
- [x] Add the first channel-aware pilot on public read-path using `rustok-channel` module bindings.
- [x] Extend the pilot to the first publication-level proof point through metadata-based `channelSlugs` allowlist on pages.
- [x] Compare the richer `pages` publication-level pilot against `blog` and keep the metadata-based model for v0 instead of introducing a dedicated relation/table yet.
- [ ] Promote `body.format=grapesjs_v1` as the canonical visual page-builder contract while keeping legacy block APIs migration-compatible.
- [ ] Standardize cross-module integration points and events.
- [ ] Document ownership and release gates for new capabilities.

### Phase 3 — Productionization (planned)

- [ ] Finalize rollout and migration strategy for incremental adoption.
- [ ] Complete security/tenancy/rbac checks relevant to the module.
- [ ] Validate observability, runbooks, and operational readiness.

## Dedicated page-builder track

This section isolates the visual page-builder work as a dedicated `rustok-pages`
track. It is intentionally separate from OAuth/app-registration work and separate
from the blog/forum rich-text rollout.

### Goal

Adopt a single visual page-builder model that:

- uses `GrapesJS` as the primary editor runtime;
- stores canonical page-builder state in `body.format = "grapesjs_v1"`;
- remains frontend-agnostic enough for both `apps/next-admin` and future
  `apps/admin`/Leptos integration;
- preserves a safe migration path from legacy typed block editing.

### Canonical direction

- `grapesjs_v1` becomes the source of truth for newly edited visual pages.
- `content_json` stores canonical GrapesJS `projectData`.
- legacy `blocks` remain supported as a migration-compatible surface until
  storefront rendering and editorial workflows are fully migrated.
- `Tiptap` stays limited to blog/forum rich-text and is not reused as a pages editor.

### Proposed implementation phases

#### Phase PB0 — Contract freeze

- [ ] Freeze `grapesjs_v1` as the canonical page-builder body format.
- [ ] Document whether legacy `blocks` are read-only, write-compatible, or deprecated per flow.
- [ ] Define a compatibility policy for existing pages without `body.format = "grapesjs_v1"`.
- [ ] Document acceptable `projectData` validation boundaries in `rustok-core`.

#### Phase PB1 — Backend contract hardening

- [ ] Ensure create/update/read/publish paths in `rustok-pages` treat `grapesjs_v1`
  as a first-class contract.
- [ ] Keep GraphQL/REST surfaces aligned for page body operations.
- [ ] Add contract tests for round-tripping GrapesJS `projectData`.
- [ ] Explicitly document the role of legacy block endpoints during migration.

#### Phase PB2 — Next Admin runtime

- [ ] Run `PageBuilder` in `apps/next-admin` as the primary pages editor.
- [ ] Support load/save flows against canonical `grapesjs_v1` payloads.
- [ ] Expose validation/load/save failures as actionable UX states.
- [ ] Confirm page selection, draft editing, and publish workflows work with real entities.

#### Phase PB3 — Storefront rendering strategy

- [ ] Decide how `grapesjs_v1` is rendered in storefront runtimes.
- [ ] Choose between direct runtime rendering, server-side transform, or publish-time export.
- [ ] Define preview behavior and parity expectations for `apps/storefront` and `apps/next-frontend`.
- [ ] Document constraints around tenant assets, CSS isolation, and embeds.

#### Phase PB4 — Leptos parity

- [ ] Define how `apps/admin` consumes the same canonical payload without creating a divergent page model.
- [ ] Decide whether Leptos embeds GrapesJS or uses a compatible shell/editor strategy.
- [ ] Align permissions, navigation, and validation behavior across `apps/next-admin` and `apps/admin`.

#### Phase PB5 — Migration and rollout

- [ ] Define migration rules for legacy pages edited through block APIs.
- [ ] Mark whether new pages may still be created through legacy block flows.
- [ ] Prepare rollout sequencing for editor enablement, storefront rendering, and deprecation of legacy paths.
- [ ] Document rollback strategy if GrapesJS-backed pages regress rendering or publishing.

### Open questions

- Should storefronts render GrapesJS `projectData` directly, or should the server
  normalize/export it into a stricter runtime format?
- Do we need a publish-time HTML/schema artifact in addition to stored `projectData`?
- At what milestone do legacy typed block writes become deprecated for new pages?
- Do we need mixed-mode support where a page can temporarily keep both legacy
  `blocks` and canonical `grapesjs_v1` body data?

### Risks

- A frontend editor can become the accidental source of backend truth unless the
  `grapesjs_v1` contract is clearly defined and versioned.
- Storefront rendering may drift if `projectData` semantics are not normalized.
- Keeping legacy `blocks` too long can create dual-write ambiguity for editors and consumers.
- Leptos parity may lag if the canonical payload is defined around Next-only assumptions.

### Test plan

- Unit:
  - validate accepted/rejected `grapesjs_v1` payload shapes;
  - verify page body round-trip for `grapesjs_v1`;
  - verify legacy non-GrapesJS pages still read safely.
- Integration:
  - create/update/get/publish page with `body.format = "grapesjs_v1"`;
  - ensure GraphQL and REST expose equivalent behavior;
  - verify RBAC enforcement on editor and publish actions.
- UI:
  - load existing GrapesJS page in `apps/next-admin`;
  - save dirty state back to canonical payload;
  - surface validation and transport failures clearly.
- Rollout:
  - confirm behavior for legacy pages with blocks only;
  - confirm no data loss when first converting a page into `grapesjs_v1`;
  - confirm storefront fallback or explicit incompatibility is documented before broad rollout.

### Definition of done

- new and edited visual pages persist through `grapesjs_v1`;
- read/write/publish flows are covered by tests;
- storefront rendering strategy is implemented and documented;
- parity plan for Leptos exists and is tracked;
- legacy block-path status is explicit: supported temporarily, restricted, or deprecated.

## Tracking and updates

When updating `rustok-pages` architecture, API contracts, tenancy behavior, routing,
or observability expectations:

1. Update this file first.
2. Update `crates/rustok-pages/README.md` and `crates/rustok-pages/docs/README.md` when public behavior changes.
3. Update `docs/index.md` links if documentation structure changes.
4. If module responsibilities change, update `docs/modules/registry.md` accordingly.

## Checklist

- [x] контрактные тесты покрывают все публичные use-case.
- [x] `pages` стал первым pilot consumer для `rustok-channel` через public read-path gating.
- [x] `pages` получил первый publication-level pilot: `channelSlugs` allowlist в metadata и channel-aware public visibility.
- [x] Сравнить metadata-based publication semantics в `pages` с более простым module-level gating в `blog`.
- [x] Решить после сравнения, сохраняем ли metadata-based модель или выносим публикацию по каналам в отдельную relation/table.

