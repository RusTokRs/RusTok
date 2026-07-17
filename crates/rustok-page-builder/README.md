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
