# rustok-page-builder

## Purpose
`rustok-page-builder` is the FBA-first visual authoring capability and compatibility module for RusTok. It owns the canonical `grapesjs` capability contract and publishes an optional Fly-based Leptos admin surface without taking ownership of consumer documents.

## Responsibilities
- keep vendor-neutral builder contract baseline (`grapesjs` write/read semantics);
- expose module runtime identity, permissions, rollout, health, validation, persistence and rendering seams;
- integrate the framework-neutral `fly` engine with Page Builder backend adapters;
- publish the optional `rustok-page-builder-admin` full-authoring composition surface;
- preserve consumer ownership of Pages, Blog, Forum and other domain document lifecycles.

## Entry points
- `src/lib.rs` — module runtime metadata (`PageBuilderModule`) and permission surface;
- `src/service.rs` — transport-neutral capability service, rollout guard, and authorized handler seam;
- `src/adapters.rs` — canonical transport adapters and Fly project inspection;
- `admin/src/lib.rs` — no-prop generated host entrypoint plus explicit consumer-owned controller entrypoint;
- `rustok-module.toml` — FBA provider and manifest-backed admin UI contract;
- [`docs/README.md`](./docs/README.md) — live runtime and integration contract;
- [`docs/implementation-plan.md`](./docs/implementation-plan.md) — module-local delivery plan.

## Interactions
- consumed by `rustok-pages` and other layout/content modules through canonical capability envelopes;
- mounted by `apps/admin` through generated manifest composition;
- delegates canonical project semantics to `fly`, presentation state to `fly-ui`, and browser/Leptos lifecycle to `fly-leptos`;
- aligned with the central rollout plan in `docs/modules/page-builder-implementation-plan.md`.

## Known Limitations
- The browser editor now has a Site symbols panel: convert a selected component
  into a shared definition and insert references through `EditorCommand`.
  `rustok-pages` synchronizes the catalog per tenant/locale and expands current
  definitions on reviewed publish. A page must be reloaded, saved and re-reviewed
  after shared definitions change; previously published artifacts never change
  until that page is explicitly re-published. The usage query is available via
  `siteSymbolUsage` GraphQL, but usage is not yet displayed in this panel;
  per-instance overrides and bulk re-publish are not implemented (see
  `DECISIONS/2026-10-09-site-symbols-shared-definitions.md`).
- The admin asset section browses and uploads media through a host-bound
  `AssetProviderPort`. The `rustok-pages` builder host binds the port to
  `rustok-media` admin dispatchers (auth context is captured by the host
  adapter); hosts that never bind the port keep the manual asset form only.
- The media panel renders on `wasm32` builds only (it uses the browser File
  APIs); SSR renders the manual asset form without the panel.
- `srcset`/`sizes` pass static validation in `static_publish_policy` (bounded
  grammar, cap-covered by the policy hash). The default policy permits validated
  `srcset`; operators can explicitly ban it through `forbidden_attributes`.
  After the policy hash changes, exact
  rebuild of earlier retained sanitized sources reports hash drift until the
  page is re-published (fail-closed by design, see
  `DECISIONS/2026-10-09-page-builder-media-asset-provider.md`).
