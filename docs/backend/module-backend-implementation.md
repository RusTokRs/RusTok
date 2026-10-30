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

## Scaffold profiles and generation contract

This section defines the contract for `cargo xtask module new` and
`cargo xtask scaffold`. A non-dry-run native scaffold now runs
`cargo xtask module lint <slug>` followed by
`cargo xtask module validate <slug>` after registration unless
`--skip-validate` is explicitly supplied. Static architecture and
publish-metadata checks are therefore enforced before a successful scaffold
result; compilation, behavior, and runtime evidence remain distinct checks.

| Profile | Canonical owner | Required baseline | Must not inherit |
|---|---|---|---|
| Native module | `modules.toml`, module-local `rustok-module.toml` and `RusToKModule` | Cargo manifest, thin `src/lib.rs`, `src/module.rs`, owner README, local docs and contract tests | unimplemented domain/persistence/ports/UI merely to fill directories |
| Support crate | actual consumer/owner, without module lifecycle by default | Cargo manifest, narrow public API and owner docs | module manifest, native registration, synthetic migrations |
| Standalone WASI | `rustok-module-template` Component Model workflow | guest/WIT contracts and its separate packaging | native `module.rs`, native host wiring or SeaORM setup |

The native scaffold command accepts optional `--profile native` and rejects
`--profile support` and `--profile wasi`. The other profiles are **not**
supported modes of the native renderer; the WASI template remains separate.

For a newly generated native module, only `src/lib.rs` (facade) and
`src/module.rs` (metadata, migrations and runtime composition) are required
runtime source files at the root. Crate-private contract tests belong in
`src/tests/contract.rs` with a `src/tests/mod.rs` entrypoint; public API
integration tests belong in top-level `tests/`. Put persistence entities in a
private `src/entities/` module without public re-exports. Do not emit empty
responsibility slots. Backend source files have the reference ceiling of
32 KiB; any exception requires an explicit documented rationale.

Additional slots depend on **implemented responsibilities**:

- `domain/` — actual state machines and invariants, independent of persistence and transports;
- `dto/` — real owner command, query and response types;
- `entities/` and `migrations/` — actual owner mappings and concrete schema changes;
- `services/` — executable use cases and transactional ownership;
- `ports/` — stable cross-owner contracts with a real consumer;
- `integrations/` — concrete adapters to another capability;
- `graphql/` — published GraphQL mapping into owner services/ports;
- `controllers/` or `rest/` — published HTTP transport mapping.

Error and event modules appear only with real owner error/event contracts.
A file containing only a comment, a no-op adapter, speculative search
projection, invented records table, or fake CRUD is **not** a layer.

`--no-db` must produce no entities, migrations or durable storage claims. A
DB request authorizes persistence only once an actual owner schema and
migration contract exist. `--no-ui` must produce no UI packages or advertised
routes. `--ui admin|storefront|dual` requests UI, not an entitlement to publish
a Leptos/Next.js route without a working backend and transport contract.

**Created** means the files and requested composition were written;
**verified scaffold** requires successful appropriate compilation, module
registration, structural validation and tests. These do **not** imply
implemented business operations, FBA `boundary_ready` /
`transport_verified` or FFA `parity_verified`. Record actual readiness in
the owner implementation plan and only project evidenced states into the
registry. Known gaps must be declared in the owner README/plan. The future
generator must preflight a complete edit plan, show all edits in dry-run,
roll back failed writes, and never return success after failed validation.

### Current implementation gap

The existing `cargo xtask scaffold` renderer emits a minimal native runtime
and internal contract tests without invented services, schema, migrations or
business operations. `--with-db` remains unsupported until an owner schema
exists. `--ui admin|storefront|dual` now creates unmounted Leptos FFA
development packages within the Cargo workspace, with independent core/model,
a shared transport-selection facade, two explicitly unavailable owner adapters,
`ui/leptos.rs`, i18n and contract tests.
The module manifest is `future_ui` with **no** live `[provides.*_ui]`.
The central readiness board remains `not_started` even though source shape
is `core_transport_ui`. No Leptos host route is mounted.
Classic Next.js packages belong to their own hosts and are not FFA.
Successful publish metadata validation does not imply runtime or UI readiness.
Atomic rollback, complete manifest-edit dry-run and an executable golden
generation matrix remain future improvements.

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
    error.rs or error/             owner error contract + public mapping when needed
    domain/                        state machines, value objects, invariant/policy code
    dto/                           owner request/response and command/query data contracts
    entities/                      persistence mappings when the module owns tables
    services/                      application/domain use cases and transactions
    ports/                         owner-defined cross-boundary ports, only when consumed
    integrations/                  adapters to SEO/reactions/index/translation/etc.
    migrations/                    owner migrations and dependency descriptors
    graphql/                       owner GraphQL adapters when published
    controllers/ or rest/          owner HTTP adapters when published
    runtime.rs                     optional narrow reusable runtime state; not admitted by the
                                   enrolled reference profile, which keeps the root at
                                   lib.rs/module.rs
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
An enrolled profile admits only `lib.rs` and `module.rs` at the source root: the
verifier fails on any other root-level `.rs` file, so `src/runtime.rs` is not part of
the reference layout. A runtime handle bundle that grows out of `module.rs` moves into
an existing responsibility slot instead of a new root file — `rustok-blog` keeps its
`runtime`-gated module-work registrations in `src/services/scheduler.rs`.

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
| Module runtime handle bundle | `src/runtime.rs` in the general layout; the enrolled reference
profile admits only `lib.rs` and `module.rs` at the source root, so a bundle that grows out of
`module.rs` moves into an existing responsibility slot |
| Capability integration adapter | `src/integrations/` |
| GraphQL root/resolver/DTO owned by the module | `src/graphql/` |
| REST handler/DTO owned by the module | `src/rest/` or `src/controllers/` |
| OpenAPI/GraphQL/FBA evidence artifact | `contracts/` |
| Module-local backend roadmap and FFA/FBA status | `docs/implementation-plan.md` |
| Maintenance command provider | `cli/` adapter package |
| Server route mounting only | `apps/server` |

If a file starts mixing two rows from the table, split it before adding new behavior.

## Static native architecture verification

The supported static entrypoint for a registered local native module is
`cargo xtask module lint <slug>`. It invokes
`scripts/verify/verify-module-source-layout.mjs --module <slug>`, which
runs the generic rules in `scripts/verify/native-module-layout.mjs`.
Blog-only reference assertions remain separate and must not become universal
requirements.

The generic lint checks source layout, Cargo/module identity, entry type,
registration placement, private persistence entities, test location, dependency
direction, bounded Rust file sizes, FFA package structure, and consistency
between `module.ui_classification`, UI package presence and
`[provides.*_ui]`. A `future_ui` package remains an unmounted draft, not a
live UI surface. `no_ui` forbids UI packages, while `admin_only`,
`storefront_only` and `dual_surface` require the corresponding actual
package and live manifest declaration.

The generic lint recognizes all six manifest classifications, including
`capability_only`, and distinguishes live UI surfaces, unmounted
`future_ui` drafts, and manifest-declared UI support sub-crates. Support
sub-crates are not routes and do not inherit the surface package layout.
Unknown source directories, empty or unwired responsibility slots,
thin backend/FFA facades, optional `runtime.rs` / `error.rs` wiring,
transport-neutral DTOs and ports, persistence-only entity dependencies,
framework-independent FFA models/core, symlinked module paths/source trees,
comment-only dependency/registration markers, and UI components that bypass
their transport facade fail closed.

There is one architecture-lint owner: `native-module-layout.mjs`.
The former standalone `verify-native-module-architecture.mjs` duplicate has
been removed; its applicable checks now live in the shared validator.
`module validate` still checks publish metadata and registration, not
architecture. Static lint does not prove compilation, tests, native or
GraphQL transport execution, Next.js parity or FBA/FFA readiness. Those
require independently recorded evidence.

```bash
cargo xtask module lint <slug>
cargo xtask module validate <slug>
node --test scripts/verify/native-module-layout.test.mjs
```

## Canonical reference invariants

Physical conformance is necessary but not sufficient. Enrolled reference modules
also follow [the canonical native module reference contract](../../DECISIONS/2026-09-18-canonical-native-module-reference-contract.md).

In particular:

- request/runtime tenant context is authoritative on reads and writes;
- runtime locale fallback never becomes stored write provenance;
- nullable edits use explicit `Patch<T>` semantics;
- aggregate revisions are predecessor-bound CAS inputs, not optional hints;
- persistence entities are private, with at most one explicitly documented bridge exception per
  module that must be pinned by its layout/reference verifier and tracked in the module cursor
  (`rustok-blog`: the orchestration bridge surface in `src/ports/`, finding L-4);
- lifecycle commands execute the domain transition table;
- derived projections do not mutate owner business revisions;
- public transports use owner-safe/redacted error mapping;
- integrations call owner service/port seams instead of rebuilding policy from entities;
- pagination is deterministic and every published input field has behavior.

### Module-owned UI adapter profile

Module-owned `admin/` and `storefront/` crates use a UI-specific profile rather
than pretending to be backend domain crates:

```text
src/
  lib.rs
  model.rs
  i18n.rs
  core/
    commands.rs
    presentation.rs
    tests.rs
  transport/
  ui/
    components.rs
    leptos.rs
```

Only responsibilities that exist are materialized. The Blog reference uses a
40 KiB source ceiling for module-owned UI files; split by responsibility, never
by arbitrary numbered chunks. UI translation bundles describe available
interface copy, not allowed content locales.

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
