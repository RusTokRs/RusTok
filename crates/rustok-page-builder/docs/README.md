# rustok-page-builder runtime

`rustok-page-builder` is the framework-neutral Page Builder module boundary.

## Core rule

The module has one current API and one current domain model. Their version is the crate/module
semver (`CARGO_PKG_VERSION`). Documents, commands, artifacts, component manifests, capabilities,

The API evolves additively during a module major. Existing compatibility fields and entrypoints stay
operational until the next module major, where they may be removed after consumers have migrated.
This additive-then-major-cleanup cycle is the permanent evolution model.

## Purpose

- provide a reusable visual Page Builder boundary before and during integration with `pages` and
  other consumer modules;
- keep authoring semantics independent of React, Leptos, Dioxus, mobile and transport choices;
- centralize capabilities `preview`, `tree`, `properties` and `publish`;
- provide rollout, health, permissions, validation, rendering and observability seams.

## Ownership

- `fly` owns the current project model, editor commands, validation, registries and deterministic
  rendering;
- `rustok-page-builder` owns capability, service, authorization, transport, rollout and the
  framework-neutral browser host contract;
- consumer modules own persistence and lifecycle;
- UI adapters only render the shared browser module source and translate framework events to the
  same Fly intents and commands.

GrapesJS is an external compatibility format and behavioural reference. It is not a version of the
RusTok Page Builder domain model.

## Current entrypoints

- `src/browser_host.rs` — framework-neutral browser module composition, safe inline config escaping
  and lifecycle-bound SSR controls reusable by Leptos and future Dioxus hosts;
- `src/dto.rs` — versionless `PageBuilderModuleMetadata`, capability DTOs and typed error catalog;
- `src/adapters.rs` — `FlyProjectInspection::decode_current` and framework-neutral endpoint seams;
- `src/adapters/fly_service.rs` — current Fly-backed service implementation;
- `src/landing.rs` — landing inspection and static-build boundary without a schema selector;
- `src/landing_service.rs` — preview/publish validation decorator;
- `src/runtime_telemetry.rs` — versionless runtime operation evidence;
- `src/service.rs` — capability service trait plus original reference/provider compatibility APIs;
- `src/transport.rs` — canonical GraphQL, Leptos server-function and future adapter envelopes;
- `src/health.rs` — provider health and SLO evidence;
- `src/rollout.rs` — capability rollout and fallback policy.

## Current runtime flow

```text
fly-browser asset + rustok-page-builder::browser_host
                         |
                         v
            Leptos / future Dioxus renderer
                         |
                         v
browser / transport adapter -> PageBuilderCapabilityRequest
                         |
                         v
               FlyProjectInspection
                         |
               +---------+---------+
               |                   |
               v                   v
          validation          registry check
               |                   |
               +---------+---------+
                         |
                         v
                Fly domain document
                         |
               +---------+---------+
               |                   |
               v                   v
            preview          static publish gate
```

No current step branches on a document version. Browser host policy is composed once in the core
crate; framework adapters do not duplicate auto-mount, form binding or lifecycle cleanup logic.

## Compatibility surface

call version-selector decode methods. These surfaces are compatibility adapters only:

- current services ignore it;
- Pages publish does not gate on it;
- Fly's domain model never receives it;

constants remain available for existing consumers during the current module major. New code must use
`PageBuilderModuleMetadata`, `decode_current`, `inspect_current` and the Fly-backed service.

## Runtime telemetry

`PageBuilderRuntimeCallEvidence` records:

- module slug;
- operation (`load_project`, `save_project`, `render_preview`);
- status (`started`, `succeeded`, `failed`);
- tenant, page, revision and correlation identifiers;
- typed error kind and stable error code.

It intentionally contains no contract, schema or payload version. The deployed module version comes
from build/runtime module metadata.

The old `PageBuilderAdapterCallEvidence` remains part of the compatibility service surface until a
module-major cleanup.

## Permissions

| Capability | Required permission | Port semantics |
|---|---|---|
| `preview` | `pages:read` | read deadline |
| `tree` | `pages:read` | read deadline |
| `properties` | `pages:update` | read deadline |
| `publish` | `pages:publish` | write deadline and idempotency key |

`pages:manage` is the effective override for all capabilities.

## Fallback profiles

| Profile | Preview | Tree/properties | Publish | Read/storefront paths |
|---|---|---|---|---|
| `all_on` | available | available | available | stable |
| `publish_off` | available | available | disabled | stable |
| `preview_off` | disabled | available | disabled | stable |
| `builder_off` | disabled | disabled | disabled | stable |

## Verification

- `cargo test -p fly landing_contract`;
- `cargo test -p rustok-page-builder --lib`;
- `cargo xtask module validate page_builder`;
- `node scripts/verify/verify-fly-ssr-first.mjs`;
- Page Builder verification scripts under `crates/rustok-page-builder/scripts/verify`.

Verification and registry scripts must treat module semver as the version boundary and must not
create new domain/payload version sources.

## Related documents

- `DECISIONS/2026-07-13-fly-page-builder-architecture.md`;
- `docs/modules/page-builder-implementation-plan.md`;
- `crates/rustok-pages/docs/implementation-plan.md`.
