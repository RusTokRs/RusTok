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

Native platform modules use one responsibility-oriented physical vocabulary. Small
modules omit responsibilities they do not have; empty scaffolding is forbidden.
Large responsibilities become nested submodules below the owning slot rather than
new top-level naming conventions.

```text
crates/modules/rustok-<module>/
  Cargo.toml
  README.md
  CRATE_API.md                      public Rust owner contract when the crate publishes one
  rustok-module.toml
  contracts/                       published FBA/OpenAPI/GraphQL evidence, no runtime code
  docs/
    README.md
    implementation-plan.md
  src/
    lib.rs                         crate facade: docs, declarations, deliberate re-exports
    module.rs                      RusToKModule/MigrationSource/runtime registration
    error.rs                       owner error contract
    domain/                        state machines, value objects, invariant/policy code
    dto/                           owner request/response and command/query data contracts
    entities/                      persistence mappings when the module owns tables
    services/                      application/domain use cases and transactions
    ports/                         owner-defined cross-boundary ports, only when consumed
    integrations/                  adapters to SEO/reactions/index/translation/etc.
    migrations/                    owner migrations and dependency descriptors
    graphql/                       owner GraphQL adapters when published
    controllers/ or rest/          owner HTTP adapters when published
    runtime.rs                     optional narrow reusable runtime state
    tests/                         crate-private contract tests needing private access
  tests/                           public-API integration tests
  admin/                           optional module-owned Leptos admin UI adapter package
  storefront/                      optional module-owned Leptos storefront UI adapter package
  cli/                             optional external CLI adapter package
```

The responsibility meaning is normative:

| Responsibility | Canonical slot | Dependency rule |
|---|---|---|
| Module metadata, migration export, runtime capability/listener registration | `src/module.rs` | Composition only; no business policy |
| Pure domain state/invariants/policies | `src/domain/` | Must not depend on SeaORM, Axum, GraphQL, Leptos or `rustok-web` |
| Owner application/use-case logic | `src/services/` | May use domain/DTO/entity/port contracts; must not depend on transport frameworks |
| Stable cross-owner API | `src/ports/` | Add only for a real consumer boundary |
| Persistence mappings/migrations | `src/entities/`, `src/migrations/` | No transport ownership |
| Capability adapters owned by the module | `src/integrations/` | Adapt owner services/ports to another capability; do not become a second source of truth |
| GraphQL/HTTP | `src/graphql/`, `src/controllers/` or `src/rest/` | Thin mapping into owner services/ports |
| Crate facade | `src/lib.rs` | Documentation, module declarations and deliberate public re-exports only |

A responsibility may become a directory of feature submodules when it grows. Split
by stable behavior (for example commands, queries, repository/projection, policy)
rather than arbitrary numbered files. For modules enrolled as canonical references,
the layout verifier also keeps individual Rust source files bounded; the initial
reference profile uses 32 KiB and requires a documented verifier exception for a
larger file.

Do not add generic root dumping grounds such as `common.rs`, `misc.rs` or
`utils.rs` for unrelated behavior. A helper belongs with the responsibility whose
semantics it implements.

`rustok-blog` is the first canonical native-module reference implementation.
Its physical layout is checked by `npm run verify:module-source-layout`. Existing
modules adopt the target incrementally in bounded mechanical PRs and become strict
when enrolled in that verifier; historical layout is not an alternative standard.

Shared/support libraries follow the same responsibility principles but are not
forced into module-only concepts. A shared crate keeps a thin `lib.rs` and uses
explicit `model/` or `types/`, `service/` or `runtime/`, `ports/`, and
`adapters/`/`integrations/` only when those responsibilities exist. It must
not invent `module.rs`, `rustok-module.toml`, migrations, or module lifecycle
registration merely to resemble a platform module.

The existing `rustok-module-template` renders standalone WASI Component Model
modules, not native server modules, and is not the reference for this physical
layout.

The `cli/`, `admin/` and `storefront/` directories are ownership-local
adapter packages. They sit next to the module for discoverability but are not part
of the domain crate. A production server build must be able to include the module
domain crate without linking the module CLI adapter.

Use these placement rules:

| Thing Being Added | Place It In |
|---|---|
| Domain state machine, value object or invariant | `src/domain/` |
| Owner DTO/command/query data contract | `src/dto/` |
| Application/domain service or transactional use case | `src/services/` |
| Cross-module stable backend port | `src/ports/` |
| Domain event | owner service/event module under the narrowest responsibility; extract `src/events/` when it becomes a real family |
| Module/runtime registration | `src/module.rs` |
| Module runtime handle bundle | `src/runtime.rs` |
| Capability integration adapter | `src/integrations/` |
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

The active host is Axum-only; module routes use the manifest-declared
`axum::Router` entrypoint and `rustok-web` response helpers.

Use `rustok-web::HttpError` / `HttpResult` only for HTTP boundary errors. Domain errors
belong to the module and should be mapped at the adapter boundary.

`rustok-web` owns response/error helpers, not a new business policy layer. It
may format JSON, status codes and HTTP envelopes. It must not
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

Documentation must describe the actual code state and the active architecture.
