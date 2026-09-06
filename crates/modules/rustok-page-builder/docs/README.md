# `rustok-page-builder` Documentation

## Purpose

`rustok-page-builder` is the Fly visual document authoring provider and runtime host for visual document editing.

## Scope

- Framework-neutral visual builder container and message bridge;
- Component contribution schema validation and preview iframe sandboxing;
- Inline authoring permissions, asset transport, and save pipeline.

## Integration

- Host for consumer modules (Pages, Blog, Forum) providing visual blocks;
- Communicates via Fly message bridge contracts.

## Verification

- `cargo test -p rustok-page-builder`
- `cargo xtask module validate page_builder`

## Related documents

- [Crate README](../README.md)
- [Implementation Plan](./implementation-plan.md)

# rustok-page-builder runtime

`rustok-page-builder` is the framework-neutral Page Builder module boundary.

## Core rule

The module has one current API, one current service path and one current domain owner. The crate
semver (`CARGO_PKG_VERSION`) is the module version. Fly owns the project document model; Page
Builder does not maintain a parallel schema or provider implementation.

GrapesJS is an import/export format and behavioural reference. It is decoded at the Fly adapter
boundary and never becomes a second Page Builder domain model.

## Ownership

- `fly` owns project decoding, validation, registries, commands, deterministic rendering and
  runtime-scenario release semantics;
- `rustok-page-builder` owns capability DTOs, service ports, authorization, rollout guards,
  transport envelopes, runtime telemetry and the framework-neutral browser host contract;
- consumer modules own persistence and publication lifecycle;
- UI adapters only render/bind framework-specific surfaces to the same Page Builder contracts.

## Current service path

`FlyAdapterBackedPageBuilderService` is the only service implementation owned by this crate.
`compose_fly_page_builder_handlers` is the default server composition root; the configured variant
accepts a preconfigured Fly service, explicit port policies and an explicit authorizer.

```text
PageBuilderCapabilityRequest
            |
            v
FlyAdapterBackedPageBuilderService
            |
            v
CapabilityGuardedService
            |
            v
AuthorizedPageBuilderHandlers
            |
            v
GraphQL / Leptos server-function envelope
```

The service uses these framework-neutral ports:

- `PageBuilderProjectStore` — tenant-scoped load/save;
- `PageBuilderPreviewRenderingPort` — contextual preview rendering after Fly validation;
- `PageBuilderRuntimeTelemetry` — started/succeeded/failed operation evidence;
- `PageBuilderScenarioBaselineStore` — optional release baseline lookup.

`PreviewPageBuilderInput` carries `PageBuilderPreviewRuntime`, including the selected JSON runtime
context and optional scenario identity. The service validates that contract after Fly structural
validation and passes the complete canonical input to `PageBuilderPreviewRenderingPort`. Consumer
renderers therefore do not define local context or scenario parameters. `PreviewPageBuilderResult`
returns the scenario identity used for rendering so hosts can reject stale or mismatched responses.

The composition root validates rollout flags before exposing handlers. It then wraps the Fly-backed
service with `CapabilityGuardedService` for rollout and port-call policy, followed by
`AuthorizedPageBuilderHandlers` for permission checks. Consumer modules supply concrete ports but do
not choose a different service/guard order.

The transport bridge dispatches GraphQL through `dispatch_graphql_envelope` and
Leptos server functions through `dispatch_leptos_server_function_envelope`. Both
paths call the same authorized handlers and preserve the typed error kind and
stable code; future mobile adapters remain an explicit transport kind, not a
second service path.

`src/adapters.rs` is the endpoint adapter seam: host GraphQL resolvers call
`handle_page_builder_graphql_endpoint`, and Leptos server functions call
`handle_page_builder_leptos_server_function_endpoint`. These entrypoints adapt
only their boundary input/output and delegate to the canonical transport bridge.

The machine-readable boundary is
`contracts/page-builder-service-boundary.json`. The corresponding verifier rejects obsolete
reference services, the removed legacy preview port, migration decorators, manual JSON rendering
paths and composition-order drift.

## Framework-neutral browser host

`src/browser_host.rs` owns:

- the `fly_browser` adapter marker;
- safe inline JSON escaping;
- config + Fly Browser asset + host bootstrap composition;
- SSR form, selection and draft-route bindings;
- lifecycle cleanup and idempotent late manual mount binding.

`crates/rustok-page-builder/admin/src/ui/browser_adapter.rs` is a thin Leptos renderer over this
source. A future Dioxus renderer can use the same source without copying browser policy.

## Current entrypoints

- `src/dto.rs` — capability DTOs, preview runtime contract and typed error catalog;
- `src/adapters.rs` — `FlyProjectInspection` and framework-neutral endpoint payloads;
- `src/adapters/fly_service.rs` — `FlyAdapterBackedPageBuilderService`;
- `src/preview_port.rs` — canonical contextual preview rendering port;
- `src/composition.rs` — current-only server composition root;
- `src/browser_host.rs` — framework-neutral browser module source;
- `src/service.rs` — capability service, persistence port, guards and authorized handlers;
- `src/transport.rs` — canonical GraphQL and server-function envelopes;
- `src/runtime_telemetry.rs` — runtime operation evidence;
- `src/runtime_scenario_release.rs` — optional scenario release gate;
- `src/landing.rs` and `src/landing_service.rs` — static landing validation and publish boundary;
- `src/health.rs` and `src/rollout.rs` — health/SLO and capability rollout policy.

## Permissions

| Capability | Required permission | Port semantics |
|---|---|---|
| `preview` | `pages:read` | read deadline |
| `tree` | `pages:read` | read deadline |
| `properties` | `pages:update` | read deadline |
| `publish` | `pages:publish` | write deadline and idempotency key |

`pages:manage` is the effective override.

## Fallback matrix

| Profile | Preview | Tree/properties | Publish | Admin path | Read/storefront paths |
|---|---|---|---|---|---|
| `all_on` | available | available | available | editable builder | stable |
| `publish_off` | available | available | `typed_feature_disabled_error` | editable builder, publish disabled | stable |
| `preview_off` | `typed_feature_disabled_error` | available | `typed_feature_disabled_error` | properties-only editor | stable |
| `builder_off` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `typed_feature_disabled_error` | `readonly_fallback` | stable |

Disabled capabilities return the stable `FEATURE_DISABLED` code. Provider
unavailability narrows capabilities and never mounts a fallback editor.

## Verification

- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-adapter-seams.mjs`;
- `node crates/rustok-page-builder/scripts/verify/verify-page-builder-fba-baseline.mjs`;
- `cargo test -p fly`;
- `cargo test -p rustok-page-builder --all-targets --all-features`;
- `cargo xtask module validate page_builder`.

## Related documents

- `crates/rustok-page-builder/docs/fly-runtime.md`;
- `DECISIONS/2026-07-13-fly-page-builder-architecture.md`;
- `docs/modules/page-builder-implementation-plan.md`;
- `crates/rustok-pages/docs/implementation-plan.md`.
