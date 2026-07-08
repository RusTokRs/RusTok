# Backend Module Architecture

This guide defines the target backend shape for RusToK modules. It complements
[How to Write a Module in RusToK](../modules/module-authoring.md) and should be read
before adding module services, HTTP handlers, GraphQL roots, `#[server]` adapters,
maintenance commands or FBA provider contracts.

## Ownership Model

Backend ownership is split by responsibility:

| Layer | Owner | What Belongs Here |
|---|---|---|
| Domain module | `crates/rustok-<module>` | Entities, domain services, ports, events, migrations, owner-owned GraphQL/REST DTOs. |
| Module UI transport adapters | `crates/rustok-<module>/admin` or `storefront` | Leptos `#[server]` functions and UI-facing transport facades over module APIs. |
| HTTP host | `apps/server` | Axum routing, middleware, request extractors, runtime assembly and route mounting. |
| Stable API contracts | `crates/rustok-api` | `PortContext`, `PortError`, permission, locale and request contracts. |
| Executable backend foundation | `crates/rustok-runtime`, `crates/rustok-web` | Runtime helper access and Axum boundary helpers. |
| FBA metadata | `crates/rustok-fba` | Provider/consumer descriptors, backend topology and transport-profile metadata. |
| CLI contracts | `crates/rustok-cli-core` | Command/provider contracts for future `rustok-cli` and module-local `cli/` adapters. |

The host composes modules; it does not become the owner of module business logic. A module
that needs a backend capability exposes typed services, ports, events or owner-owned
transport roots from its crate.

## Backend Module Physical Shape

A module backend is a small hexagonal package with optional adapter packages around it:

```text
crates/rustok-<module>/
  src/                 domain, application services, ports, events, migrations
  contracts/           published OpenAPI/GraphQL/FBA evidence, not executable code
  docs/                local contract and implementation plan
  admin/               optional module-owned Leptos admin UI adapter package
  storefront/          optional module-owned Leptos storefront UI adapter package
  cli/                 optional external maintenance-command adapter package
```

Only `src/` is the backend domain/application crate. `admin/` and `storefront/` are UI
adapter packages. `cli/` is an external operations adapter package. These packages may live
inside the module directory for ownership and discoverability, but they are not part of the
domain core and must not be required by the production HTTP server.

Generated or curated evidence belongs in `contracts/`; local runtime plans belong in
`docs/implementation-plan.md`; platform-wide rules belong in `docs/backend/*` or
`docs/architecture/*`. Do not hide executable adapters in `contracts/` or `docs/`.

## Foundation Crate Responsibilities

Use the new backend foundation crates by purpose:

- `rustok-api`: stable contracts that may be shared by modules, hosts and UI adapters.
  Do not put executable runtime wiring or Axum helpers here.
- `rustok-runtime`: helper APIs for executable runtime access, especially typed shared
  handle lookup through `HostRuntimeContext`.
- `rustok-web`: Axum HTTP boundary helpers such as JSON response mapping and HTTP error
  envelopes. It is not a domain error crate.
- `rustok-fba`: Fluid Backend Architecture metadata and registry descriptors, not
  transport implementations.
- `rustok-cli-core`: CLI provider/request/outcome contracts. Domain crates must not depend
  on `clap`, stdout, process exit semantics or the final CLI binary.

If code is repeated in two or more backend modules or hosts, decide whether it belongs in
one of these crates before adding another local helper.

## Dependency Placement Matrix

Use this table before adding a dependency:

| Code Location | May Depend On | Must Not Depend On |
|---|---|---|
| `crates/rustok-<module>/src` | `rustok-core`, `rustok-api`, domain support crates, `rustok-fba` only when publishing descriptors | `apps/server`, `loco_rs`, `rustok-cli-core` for command execution, UI crates, `clap` |
| `crates/rustok-<module>/src/graphql` | owner services, `rustok-api`, GraphQL crates already used by the module | `apps/server` resolver DTOs, Loco context, duplicated service logic |
| `crates/rustok-<module>/src/rest` or `controllers` | owner services, `rustok-web`, narrow module runtime structs | `loco_rs::controller::format`, `AppContext`, host-only controllers |
| `crates/rustok-<module>/src/runtime.rs` | explicit handles, `rustok-runtime` helpers when repeated lookup is needed | service locator patterns, global host context |
| `crates/rustok-<module>/contracts` | schema/evidence artifacts and registry JSON | executable Rust code, command scripts, runtime wiring |
| `crates/rustok-<module>/cli` | module domain crate, `rustok-cli-core` | production server runtime, UI crates, direct stdout/exit policy in domain services |
| `apps/server` | module public entrypoints, `rustok-runtime`, `rustok-web` | module business rules, module-owned DTO ownership, CLI adapters |

When the table is not enough, prefer the narrowest crate that matches the boundary. Shared
stable contracts go to `rustok-api`; executable runtime helpers go to `rustok-runtime`; Axum
response/error helpers go to `rustok-web`; FBA metadata goes to `rustok-fba`; CLI provider
contracts go to `rustok-cli-core`.

`rustok-cli-core` is intentionally not a domain dependency by default. A module's domain
crate exposes typed services; the module-local `cli/` package adapts those services to
command descriptors and outcomes.

## Runtime Context

New backend code must not accept `loco_rs::app::AppContext`.

Use these contracts instead:

- server-owned request handlers receive `ServerRuntimeContext` or a narrow state such as
  `ServerAuthRuntime` / `ServerEmailRuntime`;
- module-owned Leptos server functions receive `rustok_api::HostRuntimeContext`;
- reusable runtime handle lookup belongs in `rustok-runtime`;
- typed shared handles must fail explicitly when absent, with an actionable error.

Graceful degradation is allowed only when it is part of the documented module contract, such
as a disabled provider or read-only mode. Silent fallback to host globals or built-ins is not
allowed.

## Transport Boundaries

Transport is selected by purpose:

- GraphQL remains the UI/headless contract.
- Leptos `#[server]` functions are the default internal data layer for Leptos module UI.
- REST is for integrations, webhooks, operational endpoints and explicit HTTP contracts.
- Health and metrics are operational surfaces.

Transport adapters are thin. They map request context, validate transport shape, call owner
services/ports and map results into the transport response. Business decisions remain in the
module service layer.

During the Loco exit, legacy `loco_rs::controller::Routes` may still exist only as a route
mounting adapter until the Axum router cutover. New response formatting should use
`rustok_web::json_response`; do not add new `loco_rs::controller::format` usage.

## FBA Contracts

Fluid Backend Architecture means module backend boundaries are explicit, typed and
discoverable:

- owner-owned ports use `rustok_api::PortContext`, `PortError`, `PortCallPolicy` and
  `PortOperationKind`;
- provider/consumer metadata uses `rustok-fba` descriptors;
- runtime metadata and evidence live in module-local `contracts/` and
  `docs/implementation-plan.md`;
- the central readiness board in `docs/modules/registry.md` must match the local status.

Do not use FBA as a reason to create empty abstraction layers. Add a port or descriptor when
there is a real cross-module, host, CLI or headless consumer boundary.

## CLI and Operations

Maintenance commands are external inbound adapters, not domain core and not production
server runtime code.

Target shape:

- module domain crate exposes typed APIs;
- module-local `cli/` adapter package, when needed, depends on the domain crate and
  `rustok-cli-core`;
- future `rustok-cli` aggregates selected providers through an explicit registry;
- `apps/server` does not depend on the CLI binary or module command adapters.

Do not add new maintenance flows through `cargo loco task` or by expanding the HTTP server
binary with command execution code.

## Forbidden Backend Patterns

- New `loco_rs` imports in module backend code.
- Module services that read `AppContext`, host globals or package-local context singletons.
- New module business logic in `apps/server/src/controllers` or `apps/server/src/graphql`.
- Ad-hoc JSON/string contracts where typed DTOs, ports or events are required.
- Package-local auth, tenant, locale, channel or RBAC fallback chains.
- CLI logic in domain crates or production server runtime.
- Compatibility wrappers, deprecated aliases or old/new dual paths unless explicitly approved
  for an external staged migration with an owner and removal deadline.
