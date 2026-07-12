# Backend Module Implementation Guide

Read this document when writing or modifying backend code for a platform module or backend
support crate.

For rationale, see [Backend Module Architecture](./module-backend-architecture.md). For
verification, see [Backend Module Verification](./module-backend-verification.md).

## Required Starting Point

Before changing backend code:

1. Read `docs/index.md`.
2. Read `docs/modules/module-authoring.md`.
3. Read this guide and the module's local `README.md` / `docs/implementation-plan.md`.
4. Check `docs/modules/registry.md` and `docs/modules/implementation-plans-registry.md`.
5. If naming changes are involved, follow the naming contract in `docs/standards/coding.md`.

## Target Module Layout

A backend module should keep these responsibilities separate. Small modules may use fewer
files, but the ownership split must remain clear.

```text
crates/rustok-<module>/
  Cargo.toml
  README.md
  rustok-module.toml
  contracts/                         published FBA/OpenAPI/GraphQL evidence, no runtime code
    fba-*.json                       provider/consumer evidence when the boundary is promoted
    openapi*.json                    owner-owned REST contract artifacts, when published
    graphql*.graphql                 owner-owned GraphQL contract artifacts, when published
  docs/
    README.md
    implementation-plan.md
  src/
    lib.rs                           module wiring and public re-exports only
    module.rs                        RusToKModule implementation
    models.rs or entity/             persistence models
    migrations/                      module-owned migrations
    services/                        domain/application services
    ports.rs                         FBA ports when there is a real consumer boundary
    events.rs                        domain event types and helpers
    graphql/                         owner-owned GraphQL roots/DTOs when published
    rest/ or controllers/            owner-owned HTTP DTOs/handlers when published
    runtime.rs                       narrow module runtime state, if needed
  admin/                             optional module-owned Leptos admin UI adapter package
  storefront/                        optional module-owned Leptos storefront UI adapter package
  cli/                               optional external CLI adapter package
    Cargo.toml
    src/
      lib.rs                         command provider exports, no domain logic
      commands.rs                    command request mapping and outcomes
```

`apps/server` mounts and composes the module. It must not become the place where module
queries, mutations, DTOs or business policies accumulate.

The `cli/`, `admin/` and `storefront/` directories are ownership-local adapter packages.
They sit next to the module so a large ecosystem of third-party modules can keep all of its
adapters discoverable in one module folder. They are not part of the domain crate. A
production server build must be able to include the module domain crate without linking the
module CLI adapter.

Use these placement rules:

| Thing Being Added | Place It In |
|---|---|
| Domain entity, invariant, service, command object | `src/models.rs`, `src/entity/`, `src/services/` |
| Cross-module stable backend port | `src/ports.rs` or `src/ports/` |
| Domain event | `src/events.rs` or `src/events/` |
| Module runtime handle bundle | `src/runtime.rs` |
| GraphQL root/resolver/DTO owned by the module | `src/graphql/` |
| REST handler/DTO owned by the module | `src/rest/` or `src/controllers/` |
| OpenAPI/GraphQL/FBA evidence artifact | `contracts/` |
| Module-local backend roadmap and FFA/FBA status | `docs/implementation-plan.md` |
| Maintenance command provider | `cli/` adapter package |
| Server route mounting only | `apps/server` |

If a file starts mixing two rows from the table, split it before adding new behavior.

## `lib.rs` and Module Wiring

`lib.rs` should expose module entrypoints and owner-owned public contracts. It should not
contain business logic, request parsing or host-specific runtime assembly.

Use `RusToKModule` for module metadata, migrations, health and runtime extension
registration. If the module provides a shared capability, register it through
`register_runtime_extensions(...)`; do not require the host to manually know every concrete
provider.

## Runtime Helpers

Use the narrowest runtime contract that fits the boundary:

- `HostRuntimeContext` for module-owned Leptos `#[server]` adapters.
- `ServerRuntimeContext` or a narrow server state for host-owned request handlers.
- module-local `*HttpRuntime` / `*GraphqlRuntime` structs for owner-owned HTTP or GraphQL
  adapters that need explicit handles.
- `rustok-runtime::require_shared` for repeated typed shared-handle lookup once a helper is
  needed in multiple backend adapters.
- `rustok-runtime::RuntimeComposition` when an external CLI provider needs a host-neutral
  composition of an optional DB, typed host handles and settings. The settings value is a
  JSON snapshot; do not make `rustok-runtime` depend on `apps/server::RustokSettings`.
- The standalone CLI bootstrap reads `RUSTOK_SETTINGS_JSON` and connects
  `RUSTOK_DATABASE_URL` or `DATABASE_URL`; a module provider must still fail explicitly when
  its required handle is absent.

Do not pass full host contexts into domain services. Convert request/runtime state at the
adapter boundary and pass explicit handles into services.

Use `rustok-runtime` only for executable runtime helper behavior. Do not move request DTOs,
port DTOs, FBA descriptors, HTTP response mapping or domain errors into it. If a helper is
used only once in a module adapter, keep it local until the second real consumer appears.

## HTTP Adapters

HTTP handlers should be thin:

1. extract state, tenant/auth/locale/channel context and request payload;
2. enforce permission through the shared RBAC/security layer;
3. call module-owned service or port;
4. map the result through `rustok-web`.

For JSON responses use:

```rust
use rustok_web::json_response;

async fn handler(...) -> crate::error::Result<axum::response::Response> {
    Ok(json_response(response_dto))
}
```

Do not add `loco_rs::controller::format` imports. The active host is already
Axum-only; module routes use the manifest-declared `axum::Router` entrypoint.

Use `rustok-web::HttpError` / `HttpResult` only for HTTP boundary errors. Domain errors
belong to the module and should be mapped at the adapter boundary.

`rustok-web` is the replacement direction for Loco response/error helpers, not a new
business policy layer. It may format JSON, status codes and HTTP envelopes. It must not
decide inventory, checkout, RBAC, tenant lifecycle or other module behavior.

## GraphQL and Server Functions

GraphQL roots, DTOs and resolver policies belong to the owning module whenever the surface
is module-owned. The host composes roots; it does not own module resolver logic.

Leptos `#[server]` adapters must:

- read host data through `HostRuntimeContext`;
- call module-owned typed APIs;
- preserve GraphQL or REST parity when the surface is public/headless-capable;
- document any native-only operator/bootstrap exception in the module plan.

Do not duplicate business logic between GraphQL, REST and `#[server]`. They should call the
same services or ports.

## Ports and FBA Metadata

Add a port when another module, host, CLI provider or external boundary needs a stable
contract. A port must define:

- typed request/response DTOs;
- `PortContext` and `PortError` mapping;
- read/write policy through `PortCallPolicy`;
- tenant, actor, locale, channel, idempotency and deadline semantics as applicable;
- provider/consumer metadata and evidence when promoted in FBA readiness.

Use `rustok-fba` for descriptors and topology metadata. Do not invent local JSON shapes that
duplicate `rustok-fba` concepts.

Keep FBA artifacts close to the module:

- source descriptors live in the module crate;
- generated/static evidence lives in `contracts/`;
- status and verification notes live in `docs/implementation-plan.md`;
- central status lives in `docs/modules/registry.md`.

Do not promote FBA status just because a descriptor type exists. Promotion requires an
actual provider/consumer boundary, error mapping, fallback policy and verification evidence.

## CLI Adapters

If a module needs operational commands:

- keep domain APIs in the module crate;
- place command adapter code in a separate module-local `cli/` package;
- depend on `rustok-cli-core` from the adapter, not from domain core;
- return machine-readable `CommandOutcome` values;
- expose a factory accepting `&RuntimeComposition` so the provider can capture DB/settings/
  handles during CLI composition;
- keep stdout, prompts, `clap` and process exit behavior outside domain services.

The HTTP server must not link module command providers into the production runtime.

The future platform CLI may aggregate many module-local command providers through an
explicit registry. It should discover or select command adapters; it should not require all
third-party module commands to be implemented inside one central crate. This preserves module
isolation while keeping executable tooling outside domain code.

## Data and Migrations

Follow the shared database and i18n contracts:

- every tenant-owned table has `tenant_id`;
- localizable display text lives in translation/body tables;
- migrations are module-owned and exported through the standard migration source;
- cross-module migration ordering is explicit through dependency descriptors;
- events use typed payloads and transactional outbox when write consistency matters.

Do not store canonical state only in audit JSON, display labels or transport-specific
payloads.

## Documentation Updates

When backend contracts change, update in the same change:

- module-local `README.md`;
- module-local `docs/implementation-plan.md`;
- central docs in `docs/architecture/*` or `docs/backend/*` when the platform contract changes;
- `docs/modules/registry.md` for FFA/FBA readiness changes;
- verification scripts when a guardrail is needed to prevent drift.

Documentation must describe the actual code state, including temporary Loco inventory that
still exists.
