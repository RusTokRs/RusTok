---
id: doc://docs/research/fluid-frontend-architecture.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Fluid Frontend Architecture for RusTok

Fluid Frontend Architecture (FFA) is a proposed RusTok architectural model for
portable web frontends, where the same frontend can operate both as an
embedded monolithic runtime and as a separate headless client without rewriting the UI layer.

The key idea of FFA: `headless` describes a deployment topology, not the identity
of the frontend application. What changes is the transport and runtime boundary, but not the components,
routing, state management, and user logic.

## Context

Modern web development typically chooses one of two models.

### Monolith

```text
Frontend
    ↓
Backend
    ↓
Database
```

The frontend is embedded in the backend runtime and ships together with the server application.
Typical examples: WordPress, Magento, Rails monoliths.

Monolith pros:

- simple deployment;
- same-origin runtime;
- short path to server-side auth, session, policy and service layer;
- fewer network and CORS-boundary problems.

Monolith cons:

- frontend is often tightly coupled to the backend runtime;
- a standalone client is hard to extract without rewriting;
- UI components start knowing too much about the deployment environment.

### Headless

```text
Frontend → API → Backend → Database
```

Frontend and backend are deployed as separate systems. Typical examples: Next.js +
GraphQL, Saleor, Medusa.

Headless pros:

- independent frontend deployment;
- convenient integration of external clients;
- explicit API boundary;
- ability to use different frontend stacks on top of one backend.

Headless cons:

- network transport becomes a mandatory part of UI architecture;
- frontend is often redesigned from scratch for an API-only runtime;
- a monolithic admin usually cannot become a standalone client;
- a headless storefront usually cannot be embedded back into the backend runtime without a separate
  implementation.

## The Problem

Most platforms that call themselves headless-compatible in practice solve only
part of the problem. They provide:

- a monolithic UI;
- an external API;
- sometimes a separate headless starter or storefront template.

But the frontend itself remains non-portable. As a result, deployment topology dictates
frontend architecture:

- the admin is tightly coupled to the backend runtime;
- the storefront is implemented separately for the headless scenario;
- frontend logic is duplicated between embedded and remote UI;
- components become transport-aware;
- migration between monolith and headless becomes a UI rewrite.

In effect:

```text
headless mode = new frontend
```

FFA proposes a different contract:

```text
headless mode = different execution topology of the same frontend
```

## Definition

**Fluid Frontend Architecture (FFA)** — an architectural model in which the same
frontend can operate as an embedded monolithic runtime and as a separate headless client
without rewriting the UI layer.

In FFA:

- frontend identity remains unchanged;
- execution topology becomes fluid;
- transport is a replaceable infrastructure detail;
- UI does not depend on whether backend code executes locally, in-process, via GraphQL,
  REST, RPC, edge runtime or another transport.

## Core Principles of FFA

### 1. Preserving UI identity

The frontend remains the same application in all operating modes:

- same components;
- same routes;
- same forms and state transitions;
- same display rules;
- same module-owned UI package.

If different UI implementations of the same surface must be maintained for monolith and headless,
the system is not truly fluid.

### 2. Transport agnosticism

The UI layer should not depend on a specific transport. Transports can be:

- local Leptos `#[server]` functions;
- in-process service calls;
- GraphQL;
- REST;
- RPC;
- edge functions;
- local-first synchronization.

A component should not directly encode knowledge that it works specifically via
GraphQL or specifically via a server function. It accesses a local frontend-facing
API layer, which selects the appropriate transport for the current runtime.

Basic pattern:

```text
UI component
  → frontend-facing API adapter
  → runtime transport selector
  → local server function / in-process service / GraphQL / REST / RPC
  → domain service
```

### 3. Runtime fluidity

A frontend can move between runtime profiles without restructuring the architecture:

- embedded monolith;
- SSR/hydrate inside the backend host;
- standalone CSR/debug;
- remote headless host;
- hybrid deployment.

The runtime profile affects transport and packaging, but should not require a new component
model.

### 4. Topology independence

Frontend behavior does not depend on:

- process boundaries;
- network locality;
- deployment topology;
- whether the service layer is in the same process or behind an API boundary.

The frontend should not know where exactly the backend code executes. It should only know
a stable application contract.

### 5. Parallel contracts instead of transport replacement

FFA does not require choosing one transport forever. On the contrary, FFA assumes that transport
can be different in different runtime profiles.

For RusTok, this is especially important: the emergence of a native Leptos `#[server]` path does not mean abandoning
GraphQL. Server functions provide a short internal path for an SSR-first monolith, while
GraphQL/REST maintain headless parity and an external contract.

## How This Looks in an Ecommerce Platform

### Monolithic mode

```text
Admin UI
Storefront UI
    ↓
Backend runtime
    ↓
Database
```

Admin UI and Storefront UI are embedded in one backend runtime. They can leverage
same-origin SSR/hydrate, server-side auth/session/policy and native internal calls.

### Headless mode

```text
Admin UI      → GraphQL/REST API
Storefront UI → GraphQL/REST API
                    ↓
                Backend runtime
                    ↓
                 Database
```

The same frontend bundles work as remote clients through the public API contract.

### Hybrid mode

```text
Admin UI      → local server functions → Backend runtime
Storefront UI → GraphQL/REST API       → Backend runtime
Partner app   → GraphQL/REST API       → Backend runtime
```

One surface can be embedded, another remote, while external clients continue
to use the API. FFA does not require that the entire platform be simultaneously only a monolith
or only a headless system.

## Why Existing Platforms Are Often Not Fluid

Many platforms support API decoupling but do not support portability of the frontend
itself.

| Platform | Headless support | One frontend in embedded and headless runtime |
|---|---|---|
| WordPress | Partial | No |
| Drupal | Partial | No |
| Shopify | Partial | No |
| Saleor | Yes | No |
| Medusa | Yes | No |

These platforms can be useful headless backends but typically do not guarantee that the
same admin or storefront surface will work unchanged both inside
a monolith and as a standalone/headless UI.

In other words, they solve:

```text
API decoupling
```

but do not solve:

```text
frontend portability
```

## Why FFA Is Important for RusTok

RusTok is designed as a modular Rust platform where the backend host, module-owned UI and
headless API should not evolve as three independent architectures. FFA provides a common language
for this direction.

For RusTok, FFA means:

- module-owned admin and storefront surfaces must be portable between host profiles;
- Leptos UI should not become `#[server]`-only just because the monolithic SSR path
  is more convenient;
- GraphQL/REST should not disappear just because a shorter native path has emerged;
- Next.js hosts and external clients should remain first-class headless consumers;
- embedded Leptos hosts should get the benefits of same-origin SSR/hydrate without
  rewriting the UI;
- runtime topology should be chosen by deployment profile, not by component structure.

Practical result:

```text
deployment architecture stops dictating frontend architecture
```

## How FFA Relates to Current RusTok UI Contracts

The current dual-path model in RusTok is already close to FFA:

- `apps/admin` and `apps/storefront` target SSR-first Leptos runtime for
  the embedded/monolith profile;
- native Leptos `#[server]` functions are used as the preferred internal data layer where
  the UI actually works inside the SSR/hydrate host;
- GraphQL `/api/graphql` remains a mandatory parallel transport contract for
  Next.js hosts, standalone/debug profiles, external clients and headless parity;
- module-owned UI packages must keep a fallback path if the surface needs to work
  outside the embedded runtime;
- `apps/server` holds server functions, GraphQL and REST as different runtime surfaces, not
  as mutually exclusive architectures.

FFA turns this practice into a more explicit architectural model: the frontend is identical,
transport is selected by runtime profile.

## Architectural Rules for RusTok

For a surface to be considered FFA-compatible, it must follow these rules.

### UI components do not choose transport directly

A component should not contain branching like `if headless { graphql } else { server_fn }`.
Transport selection should live in the adapter/data-access layer, which is part of the
frontend-facing contract of a specific surface.

### GraphQL and server functions coexist

For internal Leptos hosts, the native `#[server]` path may be preferred, but GraphQL/REST
remains a live contract for headless and standalone paths. Adding a server function is
not a reason to remove a GraphQL query, mutation or resolver if they are needed for parity.

### Routing and state remain stable

Transition from embedded runtime to headless runtime should not change:

- route semantics;
- URL-driven selection state;
- form lifecycle;
- validation model;
- authorization expectations;
- i18n contract.

### Host provides runtime context

Module-owned UI should receive runtime context from the host, not invent its own
local schemes. For example, locale, tenant, auth/session and base API endpoints should be
host-provided contracts, not a package-local fallback chain.

### Domain contracts are more important than transport contracts

Transport should be a thin layer over domain/application service semantics. If
GraphQL, REST and server function implement different business rules, FFA is violated: the frontend
becomes portable only formally.

## Why Rust Is Well Suited for FFA

The Rust ecosystem is particularly well suited for FFA because it allows expressing the difference
between domain contract and transport implementation without unnecessary runtime magic.

For FFA, useful features are:

- compile-time abstractions;
- transport-independent traits;
- shared domain types;
- strict boundaries between host, module and service layer;
- zero-cost abstraction switching;
- unified error types and DTOs between transports;
- ability to use one language for backend, SSR runtime and part of the UI ecosystem.

The RusTok stack around Leptos, Axum and async-graphql allows building a frontend where execution
topology is an implementation detail, not an architectural constraint.

### i18n and FFA

For i18n to be FFA-compatible, message resolution must be split into framework-neutral core and thin framework adapters:
- `rustok-ui-i18n` owns catalog parsing, locale normalization and fallback resolution.
- `rustok-ui-i18n-leptos` adapts that core to Leptos module UI packages.
- `rustok-ui-i18n-dioxus` must be added as a sibling adapter when Dioxus enters the workspace.

`leptos_i18n` remains Leptos-specific and must not be used by module-owned FFA UI packages. Host apps may keep it for shell/navigation until host-level FFA migration.

The key principle: i18n core must work without framework imports, so both Leptos and Dioxus UI adapters can use it.

## FFA Compatibility Criteria

A surface can be considered FFA-ready if the following conditions are met:

1. The same UI package can be mounted in an embedded host and used in a
   headless-compatible profile.
2. Components access a frontend-facing adapter layer, not a specific transport.
3. There is live transport parity for the required runtime profiles.
4. GraphQL/REST fallback actually works for standalone/headless paths, if the surface
   declares such support.
5. The native server path does not change business semantics compared to the remote API path.
6. Locale, tenant, auth and policy resolution come from the host/runtime contract.
7. The surface documentation explicitly describes supported runtime profiles and transport matrix.

## Anti-patterns

FFA is violated when the following appear in the system:

- a separate embedded admin and a separate headless admin with different UI logic;
- a GraphQL-only UI that cannot be efficiently embedded into an SSR-first monolith;
- a `#[server]`-only UI that cannot be verified or run outside the embedded runtime;
- transport-aware components;
- local i18n/auth/tenant fallback chains inside module-owned UI;
- different domain rules in GraphQL resolver and server function;
- route/state model that changes between monolith and headless deployment.

## Short Formula

Traditional model:

```text
Monolith UI ≠ Headless UI
```

FFA model:

```text
Same UI + different transport + different topology = fluid frontend
```

For RusTok, this can be formulated as:

```text
Leptos embedded runtime and headless GraphQL/REST runtime should be execution modes
of one UI architecture, not two different frontend products.
```

## Related Documents

- [Fluid Backend Architecture for RusTok](./fluid-backend-architecture.md)
- [Unified Fluid Backend Architecture Implementation Plan](./fluid-backend-architecture-unified-plan.md)
- [GraphQL and Leptos Server Functions](../UI/graphql-architecture.md)
- [Architectural Principles](../architecture/principles.md)
- [API and Surface Contracts](../architecture/api.md)
- [Routing](../architecture/routing.md)
- [SSR-first Leptos hosts with headless parity](../../DECISIONS/2026-04-24-ssr-first-leptos-hosts-with-headless-parity.md)
