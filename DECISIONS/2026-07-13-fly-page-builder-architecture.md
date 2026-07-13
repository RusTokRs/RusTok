# ADR: Fly page-builder engine and dual Page Builder surfaces

## Status

Accepted

## Context

RusTok currently uses a Next/React GrapesJS editor as the behavioural reference and stores its
project object under the `grapesjs_v1` contract. The backend `rustok-page-builder` module already
owns capability, rollout, permission, validation, preview, publish, persistence-port, and
rendering-port boundaries. The missing piece is a reusable Rust editor ecosystem that does not
make JavaScript, one UI framework, one host application, or one deployment surface the canonical
source of truth.

The editor must preserve GrapesJS projects bidirectionally while supporting a future Rust-native
visual authoring experience. Admin full authoring and storefront in-context editing have different
security, routing, transport, bundling, and release constraints. Rich-text editing already exists as
an independent capability and must not be reimplemented inside the page builder.

## Decision

The following architecture is adopted:

- `fly` owns the framework-neutral project model, lossless `grapesjs_v1` codec, component-tree
  commands, history, registries, validation, clipboard fragments, revisions, and missing-provider
  preservation.
- `fly-ui` owns framework-neutral presentation state, editor intents, policies, drag/drop outcomes,
  overlays, renderer/property-editor descriptors, and contribution contracts.
- `fly-leptos` is the first browser adapter and owns Leptos components, DOM/browser events,
  coordinate translation, hit testing, iframe integration, real-DOM overlays, and cleanup.
- `rustok-page-builder/admin` and `rustok-page-builder/storefront` remain separate optional UI
  packages. They share Fly crates but do not merge transport, security, route, or bundle ownership.
- `rustok-page-builder` backend may depend on `fly`, but never on `fly-ui`, `fly-leptos`, admin, or
  storefront UI packages.
- Consumer modules own persistence and document lifecycle. Fly emits state and commands; Page
  Builder surfaces emit intents; neither layer chooses how Pages, Blog, Forum, or Product data is
  stored.
- GrapesJS remains the behavioural and project-format reference until real captured fixtures pass
  bidirectional load/save compatibility gates.
- Rich-text content is retained as an opaque, versioned payload and edited through the existing
  rich-text capability seam.
- Dioxus support is deferred until `fly-ui` stabilizes. A future `fly-dioxus` adapter must consume
  the same neutral contracts rather than fork editor semantics.

Dependency direction is strictly:

```text
fly-ui -> fly
fly-leptos -> fly-ui + fly
page-builder admin/storefront -> fly-leptos -> fly-ui -> fly
rustok-page-builder backend -> fly
```

Forbidden directions are enforced by a repository verification script.

## Consequences

- Canonical project semantics are testable without a browser or RusTok runtime.
- Unknown providers and future GrapesJS fields remain recoverable instead of being deleted during
  migrations or when optional modules are absent.
- Admin and storefront editing can evolve independently without duplicating the editor engine.
- The first implementation carries compatibility complexity because the Rust model must retain
  fields it does not yet understand.
- Browser interaction, iframe geometry, accessibility, sanitization, and real GrapesJS captures
  remain explicit programme gates; creating the crates alone does not complete those gates.
- New dependencies in the Fly ecosystem require a dependency record and build-versus-adopt review.
