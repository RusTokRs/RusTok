---
id: doc://docs/research/fluid-backend-architecture.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Fluid Backend Architecture for RusTok

Fluid Backend Architecture (FBA) is a proposed RusTok architectural model for
portable backend modules, where the same module-owned domain/service layer
can operate both as an embedded part of a modular monolith and as a separate remote
service without rewriting business logic.

The short answer to the question "can a similar pattern be applied to the backend so that modules
can work as microservices via `server-grpc`": **yes, but FBA should describe not a
mandatory migration of all modules to microservices, but portability of the backend boundary between
in-process and out-of-process topology**.

In FBA, `microservice` is a deployment topology, not a new module identity. What changes is the
runtime boundary and transport, but not domain contract, ownership, tenancy, policy model,
observability and module lifecycle semantics.

## Relationship with FFA

Fluid Frontend Architecture (FFA) answers the question:

```text
can one frontend work embedded and headless without rewriting UI?
```

Fluid Backend Architecture (FBA) answers the symmetric backend question:

```text
can one backend module work in-process and as a remote service without rewriting
the business/application layer?
```

Comparison:

| Layer | Fluid model | What becomes replaceable | What remains stable |
|---|---|---|---|
| Frontend | FFA | `#[server]`, GraphQL, REST, RPC, edge transport | UI identity, routes, state, UX logic |
| Backend | FBA | in-process call, gRPC, HTTP/RPC, async events | module identity, service contract, domain rules |

The idea is the same: execution topology should not dictate the architectural identity
of a component.

## The Problem with Backend Modules

Typically, a platform backend evolves along one of two paths.

### Modular monolith

```text
Server process
├─ Module A
├─ Module B
└─ Module C
        ↓
     Database
```

Pros:

- simple deployment;
- short in-process call path;
- shared transactions and unified request context;
- less network complexity;
- easier to maintain typed Rust contracts.

Cons:

- operational isolation is limited;
- heavy modules cannot be scaled independently;
- failure domain is often shared;
- a module is hard to extract as a separate service if the domain/service layer is coupled to host internals.

### Microservices

```text
Server/API gateway → Service A
                   → Service B
                   → Service C
                         ↓
                  Service-owned storage
```

Pros:

- independent scaling;
- separate release/deploy cadence;
- independent failure domains;
- explicit service boundaries.

Cons:

- network boundary becomes part of the business path;
- introduces latency, retries, timeouts, compatibility windows;
- more complex transactions and consistency;
- easy to end up with a distributed monolith if boundaries do not match ownership.

## Why "Microservices First" Is Not the Goal of FBA

FBA does not say that every RusTok module should become a separate service. For the majority of
modules, an embedded modular monolith remains the best default: it is simpler, faster, cheaper to
operate and better suited for early product iteration.

FBA serves a different purpose: **design the module boundary so that remote extraction is
architecturally possible without rewriting the module**.

In other words:

```text
FBA ≠ microservices-first
FBA = service-boundary-ready modules
```

## Definition

**Fluid Backend Architecture (FBA)** — an architectural model in which the same
backend module can operate as an in-process part of a modular monolith and as a separate remote
service without rewriting the domain/application layer.

In FBA:

- module identity remains unchanged;
- the module-owned service contract remains canonical;
- transport is an adapter layer, not the owner of business logic;
- runtime topology can be embedded, remote or hybrid;
- tenant, auth, locale, channel, policy and observability context are passed through a common
  contract, not through ad-hoc headers;
- gRPC is one of the transports for backend-to-backend boundary, but does not replace
  GraphQL/REST/UI contracts.

## `server-grpc` as a Backend Transport Profile

For RusTok, `server-grpc` can be considered as an optional backend transport profile:

```text
apps/server ──in-process──> module service
apps/server ───gRPC───────> module service process
```

Importantly: `server-grpc` is not a new public API for UI and not a replacement for GraphQL/REST. Its role is
backend-to-backend communication between the host/runtime and the module-owned service boundary.

Minimal model:

```text
module domain/service trait
        │
        ├─ in-process implementation
        │
        └─ gRPC client/server adapter
```

GraphQL, REST, `#[server]` functions and UI-facing surfaces remain external
contracts of the host. They access the same module service contract, regardless of whether it
executes locally or remotely.

## FBA Topologies

### 1. Embedded modular monolith

```text
apps/server
  ├─ product module service
  ├─ order module service
  └─ forum module service
        ↓
     shared runtime/database boundary
```

This is the baseline topology for RusTok. Modules live in-process but already have explicit typed
service contracts and do not directly depend on host-specific shortcuts.

### 2. Remote module service

```text
apps/server → gRPC → product-service
apps/server → gRPC → order-service
apps/server → gRPC → forum-service
```

The host remains the composition root for public API, auth/session, tenant routing and UI-facing
contracts, but a specific module service can execute in a separate process.

### 3. Hybrid topology

```text
apps/server
  ├─ in-process: pages, blog, seo
  ├─ gRPC: search/index
  └─ gRPC: ai/recommendations
```

Some modules remain embedded, others are extracted remote. This model is the most practical:
it allows extracting only those modules where there is a real operational reason.

### 4. Async-first companion service

```text
module service → outbox/events → worker/service
```

Not every backend boundary needs to be synchronous gRPC. For indexing, email, analytics,
AI enrichment, media processing and similar tasks, the event/outbox path is often better than a request
path RPC.

## Core Principles of FBA

### 1. Preserving module identity

The module remains the same module in all topologies:

- same `slug` and ownership;
- same domain rules;
- same service contract;
- same RBAC/policy expectations;
- same documentation of runtime profiles;
- same compatibility with module lifecycle.

If a remote service requires a separate implementation of business logic, FBA is violated.

### 2. Transport agnosticism of the service layer

The application/domain layer should not know whether it was called in-process or via gRPC.

Correct form:

```text
GraphQL/REST/#[server]/job/CLI
        ↓
module service contract
        ↓
in-process impl or remote client adapter
```

Incorrect form:

```text
GraphQL resolver → in-process business logic
Grpc handler     → separate business logic
REST handler     → third business logic
```

### 3. Context propagation as a contract

A remote boundary should not turn into a set of random headers. Across an FBA boundary,
the following must be explicitly propagated:

- tenant context;
- authenticated principal/service identity;
- RBAC/policy claims;
- locale/effective language context, if the operation depends on locale;
- channel context for storefront/read-side scenarios;
- correlation/request id;
- trace/span context;
- idempotency key for retry-safe mutations;
- deadline/timeout/cancellation semantics.

### 4. Data ownership and consistency must be explicit

FBA should not hide the question of data storage. For each remote-capable module, the
supported mode must be described in advance:

| Mode | Description | When appropriate |
|---|---|---|
| Shared database, in-process | Module operates within the shared DB/schema boundary of the host | baseline modular monolith |
| Shared database, remote service | Remote process accesses the same DB with the same tenant/policy constraints | temporary extraction or controlled internal deployment |
| Service-owned database | Module owns its storage boundary and publishes read/write contracts | mature microservice boundary |
| Read-model replica | Remote service maintains a read-side projection via events/outbox | search, index, analytics, recommendations |

Transition to a service-owned database is a separate architectural decision. FBA can prepare the
contract but should not automatically promise distributed transaction semantics.

### 5. Synchronous RPC is not a substitute for events

`server-grpc` is suitable for request/response operations where the host needs an immediate answer.
For background, eventually consistent and fan-out scenarios, events/outbox are better.

Practical rule:

- command/query in request path → may be gRPC;
- workflow, projection, integration, notification → typically event/outbox;
- cross-module transaction → requires a separate design, not just "use gRPC".

### 6. Observability parity

The same service contract should provide comparable telemetry in embedded and remote
modes:

- metrics for latency/error rate;
- tracing spans on the host and service side;
- structured errors;
- health/readiness for remote service;
- version/capability negotiation;
- clear degradation path when an optional remote service is unavailable.

## Where FBA Is Especially Useful in RusTok

FBA should be applied not to all modules equally, but to those boundaries where there is a
real reason for remote topology.

Good candidates:

- search/indexing service;
- AI/recommendations/enrichment;
- media processing;
- heavy export/import and batch jobs;
- integrations/webhooks dispatch;
- analytics/reporting projections;
- high-throughput catalog read-side;
- fraud/risk scoring, if a separate lifecycle appears.

Be cautious about extracting:

- checkout/payment/order write path;
- tenant/module lifecycle;
- auth/session/RBAC core;
- i18n/locale resolution foundation;
- cross-cutting SEO metadata write path;
- any flows where strong consistency is needed without a mature saga/outbox model.

## Architectural Skeleton for RusTok

For an FBA-ready backend module, think in layers:

```text
crates/rustok-<module>/
  domain types
  application service trait
  in-process service implementation
  repository interfaces
  errors/DTOs/context contracts

crates/rustok-<module>-grpc/       optional
  protobuf/service schema
  grpc server adapter
  grpc client adapter
  context mapping
  error mapping

apps/server
  module wiring
  transport selection
  public API surfaces
```

Key point: the gRPC crate does not own business logic. It only serializes the call,
propagates context and invokes the canonical service contract.

## FBA-Ready Module Criteria

A module can be considered FBA-ready if the following conditions are met:

1. Domain/application service contract is separated from Axum, GraphQL, REST and host internals.
2. Public API handlers call the service contract rather than duplicating business logic.
3. There is a typed request context for tenant/auth/locale/channel/policy/trace data.
4. Errors have a stable mapping between domain errors and transport status codes.
5. Mutations have an idempotency/deadline/retry story if they are intended to be called remotely.
6. Cross-module dependencies are expressed through explicit ports/events, not through direct access
   to other repository internals.
7. Data ownership and consistency model are described in the module's local docs.
8. Observability and health/readiness behavior are described for a remote profile.
9. Versioning of the service contract does not break embedded and remote topology simultaneously.
10. The `server-grpc` profile can be enabled or disabled without changing the UI-facing API contract.

## Anti-patterns

FBA is violated when the following appear:

- a "microservice" with a separate copy of business logic;
- a gRPC handler as the new canonical domain owner;
- direct SQL queries from the host into tables of a remote-owned module;
- ad-hoc headers for tenant/auth/locale instead of a typed context contract;
- synchronous RPC where an event/outbox workflow is needed;
- distributed transaction without an explicit saga/compensation model;
- different RBAC/policy checks for embedded and remote mode;
- a remote service that cannot be started in-process for local/dev/test profile;
- module extraction without documentation of data ownership and failure semantics.

## Practical Conclusion

Yes, RusTok can apply a similar pattern to the backend. But it is better to formulate FBA not as
"all modules become microservices", but as:

```text
One module-owned backend contract can execute in-process or via server-grpc,
while public API/UI/event contracts continue to see the same module and the same domain semantics.
```

This gives RusTok a gradual path:

1. first build a strict modular monolith;
2. then extract service contracts and context propagation;
3. then add optional `server-grpc` adapters for suitable modules;
4. only then extract specific modules to remote deployment, if there is an
   operational reason.

## Short Formula

Traditional model:

```text
Microservice mode = new backend implementation
```

FBA model:

```text
Same module service + different transport + different topology = fluid backend
```

For RusTok, this can be formulated as:

```text
Modular monolith and server-grpc microservice profile should be execution modes
of one module-owned backend contract, not two different backend products.
```

## Related Documents

- [Unified Fluid Backend Architecture Implementation Plan](./fluid-backend-architecture-unified-plan.md)
- [Fluid Frontend Architecture for RusTok](./fluid-frontend-architecture.md)
- [Module Architecture](../architecture/modules.md)
- [API and Surface Contracts](../architecture/api.md)
- [Event Flow Contract](../architecture/event-flow-contract.md)
- [DataLoader](../architecture/dataloader.md)
- [Modular Platform Overview](../modules/overview.md)
- [How to Write a Module in RusToK](../modules/module-authoring.md)
