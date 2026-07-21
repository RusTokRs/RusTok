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
- `PageBuilderRenderingAdapter` — preview rendering after Fly validation;
- `PageBuilderRuntimeTelemetry` — started/succeeded/failed operation evidence;
- `PageBuilderScenarioBaselineStore` — optional release baseline lookup.

The composition root validates rollout flags before exposing handlers. It then wraps the Fly-backed
service with `CapabilityGuardedService` for rollout and port-call policy, followed by
`AuthorizedPageBuilderHandlers` for permission checks. Consumer modules supply concrete ports but do
not choose a different service/guard order.

GraphQL and Leptos server-function endpoints use the composed handlers and canonical envelopes.

The machine-readable boundary is
`contracts/page-builder-service-boundary.json`. The corresponding verifier rejects obsolete
reference services, migration decorators, manual JSON rendering paths and composition-order drift.

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

- `src/dto.rs` — capability DTOs and typed error catalog;
- `src/adapters.rs` — `FlyProjectInspection` and framework-neutral endpoint payloads;
- `src/adapters/fly_service.rs` — `FlyAdapterBackedPageBuilderService`;
- `src/composition.rs` — current-only server composition root;
- `src/browser_host.rs` — framework-neutral browser module source;
- `src/service.rs` — service/port traits, guards and authorized handlers;
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

## Fallback profiles

| Profile | Preview | Tree/properties | Publish | Read/storefront paths |
|---|---|---|---|---|
| `all_on` | available | available | available | stable |
| `publish_off` | available | available | disabled | stable |
| `preview_off` | disabled | available | disabled | stable |
| `builder_off` | disabled | disabled | disabled | stable |

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
