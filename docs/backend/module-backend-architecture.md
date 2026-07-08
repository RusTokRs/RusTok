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

