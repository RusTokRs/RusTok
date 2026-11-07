# Backend Module Architecture

This guide defines the target backend shape for RusToK modules. It complements
[How to Write a Module in RusToK](../modules/module-authoring.md) and should be read
before adding module services, HTTP handlers, GraphQL roots, `#[server]` adapters,
maintenance commands or FBA provider contracts.

## Ownership Model

Backend ownership is split by responsibility:

| Layer | Owner | What Belongs Here |
|---|---|---|
| Domain module | `crates/modules/rustok-<module>` | Entities, domain services, ports, events, migrations, owner-owned GraphQL/REST DTOs. |
| Module UI transport adapters | `crates/modules/rustok-<module>/admin` or `storefront` | Leptos `#[server]` functions and UI-facing transport facades over module APIs. |
| HTTP host | `apps/server` | Axum routing, middleware, request extractors, runtime assembly and route mounting. |
| Stable API contracts | `crates/libs/rustok-api` | `PortContext`, `PortError`, permission, locale and request contracts. |
| Executable backend foundation | `crates/libs/rustok-runtime`, `crates/libs/rustok-web` | Runtime helper access and Axum boundary helpers. |
| FBA metadata | `crates/libs/rustok-fba` | Provider/consumer descriptors, backend topology and transport-profile metadata. |
| CLI contracts | `crates/utils/rustok-cli-core` | Command/provider contracts for future `rustok-cli` and module-local `cli/` adapters. |

The host composes modules; it does not become the owner of module business logic. A module
that needs a backend capability exposes typed services, ports, events or owner-owned
transport roots from its crate.

## Backend Module Physical Shape

A module backend is a small hexagonal package with optional adapter packages around it:

```text
crates/modules/rustok-<module>/
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
  handle lookup through `HostRuntimeContext` and `RuntimeComposition` for host-neutral
  DB/settings/handle assembly.
- `rustok-web`: Axum HTTP boundary helpers such as JSON response mapping and HTTP error
  envelopes. It is not a domain error crate.
- `rustok-fba`: Fluid Backend Architecture metadata and registry descriptors, not
  transport implementations.
- `rustok-cli-core`: asynchronous CLI provider/request/outcome contracts. Domain crates must
  not depend on `clap`, stdout, process exit semantics or the final CLI binary.

If code is repeated in two or more backend modules or hosts, decide whether it belongs in
one of these crates before adding another local helper.

## Dependency Placement Matrix

Use this table before adding a dependency:

| Code Location | May Depend On | Must Not Depend On |
|---|---|---|
| `crates/modules/rustok-<module>/src` | `rustok-core`, `rustok-api`, domain support crates, `rustok-fba` only when publishing descriptors | `apps/server`, `rustok-cli-core` for command execution, UI crates, `clap` |
| `crates/modules/rustok-<module>/src/graphql` | owner services, `rustok-api`, GraphQL crates already used by the module | `apps/server` resolver DTOs, host context, duplicated service logic |
| `crates/modules/rustok-<module>/src/rest` or `controllers` | owner services, `rustok-web`, narrow module runtime structs | host-only response helpers, service locators, host-only controllers |
| `crates/modules/rustok-<module>/src/runtime.rs` | explicit handles, `rustok-runtime` helpers when repeated lookup is needed | service locator patterns, global host context |
| `crates/modules/rustok-<module>/contracts` | schema/evidence artifacts and registry JSON | executable Rust code, command scripts, runtime wiring |
| `crates/modules/rustok-<module>/cli` | module domain crate, `rustok-cli-core` | production server runtime, UI crates, direct stdout/exit policy in domain services |
| `apps/server` | module public entrypoints, `rustok-runtime`, `rustok-web` | module business rules, module-owned DTO ownership, CLI adapters |

When the table is not enough, prefer the narrowest crate that matches the boundary. Shared
stable contracts go to `rustok-api`; executable runtime helpers go to `rustok-runtime`; Axum
response/error helpers go to `rustok-web`; FBA metadata goes to `rustok-fba`; CLI provider
contracts go to `rustok-cli-core`.

`rustok-cli-core` is intentionally not a domain dependency by default. A module's domain
crate exposes typed services; the module-local `cli/` package adapts those services to
command descriptors and outcomes.

## Runtime Context

New backend code must not accept a host-wide service locator.

Use these contracts instead:

- server-owned request handlers receive `ServerRuntimeContext` or a narrow state such as
  `ServerAuthRuntime` / `ServerEmailRuntime`;
- module-owned Leptos server functions receive `rustok_api::HostRuntimeContext`;
- reusable runtime handle lookup belongs in `rustok-runtime`;
- CLI providers are constructed with `rustok-runtime::RuntimeComposition`, which may carry
  a DB-backed `HostRuntimeContext` and a JSON settings snapshot without importing
  `apps/server` configuration types;
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

The active server and module composition are Axum-only. New response formatting
uses `rustok_web::json_response`; do not add a second routing model.

## FBA Contracts

Fluid Backend Architecture preserves a module's identity, owner implementation and
business semantics while allowing an explicitly supported boundary to execute in-process
or through a remote adapter. The modular monolith remains the default. The active
[embedded-first plan](../research/fluid-backend-architecture-unified-plan.md#24-active-embedded-first-foundation-plan-2026-h2)
does not require a remote service for every module.

### Scope of a deployment claim

| Claim | Required scope |
|---|---|
| Embedded boundary | Named owner ports/services, their real consumers, data ownership and local execution evidence. |
| Remote capability profile | Exact supported operations, required outgoing capabilities, context/error semantics, data placement, transport, provider host and consumer-path evidence. |
| Whole-module remote deployment | Every required query, command, event handler, background task, lifecycle/migration responsibility and public surface is assigned to the remote owner or explicitly composed through its contracts. All required dependencies and transaction boundaries are resolved. |

A capability is an interface grouping, not automatically a separate process. One module
process can provide several capabilities. A read-only catalog service is a valid bounded
profile; it does not establish that Product commands, schema operations or all UI surfaces
can run remotely. Unsupported optional operations must fail explicitly. A consumer's
required capabilities must be validated before admitting that consumer path; a trait's
default `Unavailable` method is not evidence that a provider implements that capability.

For operations declared by both profiles, run the same observable contract cases against
the same owner implementation. Preserve domain results, tenant/channel/locale/policy and
typed errors; remote transport adds latency, cancellation and uncertain-outcome failures.
Do not promise that a timed-out write had no effect. Retry only under the owner idempotency
and recovery contract.

### Contract ownership and packaging

- Shared `PortContext`, `PortError`, `PortCallPolicy` and `PortOperationKind`
  belong to `rustok-api`. Domain ports, DTOs and published events belong to their module.
- Contracts and implementation may stay in one owner crate. A separate owner contracts
  package is optional when demonstrated consumers need a smaller dependency boundary;
  it does not create another domain module or tenant lifecycle entry.
- A contracts-only Cargo build is an additional packaging capability, not an FBA readiness
  requirement. It requires optional implementation dependencies, independent public types
  and checks of the resolved feature graph. Cargo features are additive; they do not choose
  a runtime provider. Product does not currently publish such a build.
- Keep the existing owner transport and standalone-service packages separate under
  `AGENTS.md` section 5. Their package count does not establish remote completeness.
- Do not create empty contract, repository or transport layers for modules without a real
  boundary. Module-specific contracts do not move to `rustok-api` merely because another
  module consumes them.

### Composition and runtime responsibility

The composition root may read deployment configuration, obtain secret handles, select an
embedded or remote provider, invoke adapter constructors and manage connection/shutdown
lifecycles. This belongs to the host or an explicit infrastructure composition adapter.
Transport packages encapsulate protocol framing, connection validation, TLS/authentication,
deadline/status mapping and transport mechanics. Domain services receive explicit
dependencies; they do not discover hosts, read deployment environment variables or choose
their own transport.

Consumers use the selected owner ports rather than constructing another provider on
failure. Explicit optional degradation is part of the consumer contract. Configuring
embedded execution is not failover from a selected remote authority.

### Storage and transactions

Process placement and storage placement are separate decisions. Controlled remote
execution may use the same physical database, subject to owner access boundaries;
autonomous service-owned storage and read-model replicas require their own deployment,
migration and recovery contracts. A separate database per port is not an FBA rule.

An interface accepting a SeaORM `DatabaseTransaction` is a native co-located
transaction-participant contract. It cannot cross RPC, even when both processes connect
to the same database. Before independently extracting its participants, either keep the
atomic collaboration co-deployed or accept an explicit new consistency/workflow design.
Outbox, idempotency, recovery and compensation must preserve the agreed business contract;
adding a saga is not automatically equivalent to the existing transaction.

### Events and observability

Use synchronous ports for operations whose caller needs a result now. Keep asynchronous
workflows, projections and fan-out on canonical events/outbox and define their delivery,
ordering, replay and recovery rules. Replacing an in-process call with RPC does not change
which events the owner must publish.

Record comparable operation latency and failures in embedded and remote profiles. Correlate
consumer/provider spans, report readiness for the configured capabilities and expose typed
degradation. Where a profile needs version or capability compatibility checks, specify how
they are enforced; metadata alone does not enforce them.

### Metadata, readiness and current examples

Provider/consumer descriptors belong to `rustok-fba`; that crate describes capabilities
and topology, but does not implement runtime discovery, routing, capability negotiation
or provider selection. Module-local `contracts/` and `docs/implementation-plan.md`
record the implemented profile and evidence. The central readiness board in
`docs/modules/registry.md` projects the same status.

Readiness always has a scope. `boundary_ready` is not whole-module remote certification;
`transport_verified` needs executed evidence for the declared provider/consumer profile.
Use the [verification guide](./module-backend-verification.md#fba-evidence) for the evidence
levels and the current static-boundary policy.

Current bounded profiles illustrate this distinction:

- Product's `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` selects three catalog projection
  RPCs and two SKU identity RPCs. Product commands remain embedded. See the
  [Product scope and gaps](../../crates/modules/rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
- `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` selects the Profiles public-image descriptor
  provider. It does not move all Media operations or image bytes to gRPC. See the
  [Media owner plan](../../crates/modules/rustok-media/docs/implementation-plan.md#ffafba-boundary).
- Whole-module isolated Media/Search deployment remains a
  [proposed pilot](../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md),
  with acceptance and operational gates distinct from existing adapters.

## CLI and Operations

Maintenance commands are external inbound adapters, not domain core and not production
server runtime code.

Target shape:

- module domain crate exposes typed APIs;
- module-local `cli/` adapter package, when needed, depends on the domain crate and
  `rustok-cli-core`;
- future `rustok-cli` aggregates selected providers through an explicit registry;
- generated provider factories receive `&RuntimeComposition`, so a module CLI adapter can
  capture the runtime it needs while remaining outside the production server build;
- `apps/server` does not depend on the CLI binary or module command adapters.

Do not add maintenance flows by expanding the HTTP server binary with command
execution code.

## Forbidden Backend Patterns

- Module services that read host-wide contexts, host globals or package-local context singletons.
- New module business logic in `apps/server/src/controllers` or `apps/server/src/graphql`.
- Ad-hoc JSON/string contracts where typed DTOs, ports or events are required.
- Package-local auth, tenant, locale, channel or RBAC fallback chains.
- CLI logic in domain crates or production server runtime.
- Compatibility wrappers, deprecated aliases or old/new dual paths unless explicitly approved
  for an external staged migration with an owner and removal deadline.
