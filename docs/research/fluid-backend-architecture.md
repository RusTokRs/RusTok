---
id: doc://docs/research/fluid-backend-architecture.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Fluid Backend Architecture for RusTok

Fluid Backend Architecture (FBA) is RusTok's model for portable, owner-defined backend
boundaries. Its target is to reuse the same domain/application implementation in a modular
monolith and in an explicitly supported remote deployment. The modular monolith is the
default; portability does not mean every module already has a remote provider.

The [backend architecture contract](../backend/module-backend-architecture.md#fba-contracts)
defines current rules. The [unified implementation plan](./fluid-backend-architecture-unified-plan.md)
owns sequencing and its active embedded-first focus. Owner plans and the central readiness
board own implementation status. This concept document does not grant an unimplemented
runtime profile or supersede a Proposed ADR's acceptance gates.

## Relationship with FFA

| Layer | Replaceable execution boundary | Stable owner contract |
|---|---|---|
| FFA | Native server functions, GraphQL or another supported UI transport | UI identity, routes, state and business scenarios |
| FBA | In-process calls, RPC or an explicit event-driven companion | Module identity, domain rules, operation semantics and data ownership |

Changing backend placement must not create a second implementation of business rules.
UI/public API contracts remain owned and maintained independently of the chosen backend
transport. Their adapters must actually use the selected owner capability before remote
parity can be claimed.

## Definition and scope

Keep three claims distinct:

1. **Embedded boundary:** an owner service/port with explicit dependencies and local behavior.
2. **Remote capability profile:** a named subset of operations with a working adapter,
   provider host, context/security boundary and consumer-path evidence.
3. **Whole-module remote deployment:** complete required reads, writes, events, jobs,
   lifecycle/migrations, outgoing dependencies and public-surface composition are accounted
   for under the remote owner, with executed evidence for the declared topology.

Several capabilities may be served by one module process. A capability or Cargo package
does not automatically become a microservice. A partial read profile is useful and valid;
it must identify unsupported operations and must not be advertised as whole-module parity.

## FBA Topologies

| Topology | Meaning | Required qualification |
|---|---|---|
| Embedded | Owner implementation executes inside a selected host process. | Explicit contracts still protect module ownership. |
| Remote | A declared capability is invoked through a network adapter. | Name operations, provider host, authority, storage and evidence. |
| Hybrid | Selected owners or capabilities are local and others remote. | Preserve one authority per operation/data boundary; define shared-storage effects. |
| Async companion | Owner facts reach a worker through the canonical event/outbox path. | Define delivery, idempotency, replay, ordering and recovery. |

`server-grpc` is conceptual shorthand for a gRPC backend profile, not a universal
implemented binary or switch. Existing Product and Media adapters publish their own
specific configuration and supported operations. They do not replace GraphQL, REST,
Leptos native functions or HTTP media-byte delivery.

## Core Principles of FBA

### Owner implementation and semantic conformance

The local provider and remote server call the same owner implementation. Adapters map
requests and errors, authenticate callers and enforce transport constraints. They do not
reimplement catalog policy, price calculation or another owner's persistence rules.

For operations supported in both profiles, execute the same observable contract cases:
business results, tenant/channel/locale/policy behavior and typed owner failures must agree.
Remote transport adds latency and failure/uncertain-outcome cases. A timeout does not prove
a write failed before commit; idempotency and recovery remain owner contracts.

### Context and authority

Owner ports use canonical `PortContext`, `PortError` and call-policy primitives from
`rustok-api`. Carry effective tenant, principal, channel, locale, correlation, trace,
deadline and mutation idempotency evidence as required by the operation.

A serialized actor or tenant is a claim, not automatically trusted authority. The remote
adapter authenticates the peer and validates or replaces claims under the provider's
documented policy. Optional degradation is explicit and observable. A selected remote
provider must not silently turn into a different local authority on failure.

### Data ownership and consistency

Process and storage placement are separate axes:

| Storage profile | Contract |
|---|---|
| Shared database, in-process | Normal modular-monolith deployment, with owner-controlled table access. |
| Controlled shared database, remote process | Process isolation without autonomous schema/data lifecycle; disclose overlapping owner access. |
| Service-owned database/schema | Explicit owner credentials, migrations, event integration and recovery. |
| Read-model replica | Derived, rebuildable state with source-version, lag and replay semantics. |

A separate physical database is not mandatory for every remote capability. Consumer SQL
against another owner's private tables remains forbidden. A read process sharing canonical
owner storage must not be described as an independently maintained replica.

Native interfaces accepting `DatabaseTransaction` describe co-located atomic collaboration.
Sharing a database URL does not make a transaction handle transferable across RPC.
Independent extraction requires an explicit decision to keep atomic participants co-deployed
or change the consistency/workflow contract. Outbox, receipts, compensation and reconciliation
must be designed for that decision; merely injecting a port or adding a saga is insufficient.

### Events and observability

Use request/response ports for work requiring an immediate result. Keep asynchronous
workflows, projections and fan-out on the canonical event/outbox path. Define how a remote
profile preserves event delivery and how operators observe request failures, latency,
readiness and degraded behavior across the consumer and provider.

### Composition and package boundaries

The host composition root selects providers, reads deployment configuration, supplies
credentials and manages resource lifecycle. Transport packages encapsulate connection,
authentication, serialization and status mapping. Domain services take explicit dependencies.

Public ports and DTOs may stay in the owner crate with local implementation. An optional
owner contracts package can reduce consumer dependencies without creating a new domain
module. A contracts-only Cargo feature profile is separate, verified packaging work;
compiling ORM code into a consumer does not by itself establish a runtime ownership violation.

Existing standalone transport/service packages remain separate under repository policy.
Create new packages and repository abstractions only for actual responsibilities.
Executable contracts belong in Rust source; module-local `contracts/` holds published
schemas, registries and evidence, not runtime code.

## Architectural Skeleton for RusTok

```text
crates/modules/rustok-<module>/
  src/                 owner contracts and canonical implementation
  rustok-module.toml   identity, dependencies and supported capability metadata
  contracts/          published schema/registry/evidence artifacts
  docs/               current contract and implementation plan

crates/modules/rustok-<module>-transport/  when a real remote profile exists
  src/                client/server adapters, context/auth/error mapping
  proto/              wire framing

crates/modules/<owner-service-package>/   when a standalone provider exists
  src/main.rs         composition, configuration, readiness and shutdown

apps/server/
  selected provider construction, public API composition and host lifecycle
```

This describes responsibilities, not mandatory empty directories or a fixed package count.
`rustok-fba` supplies metadata types and registry shape. It does not select providers,
negotiate capabilities, route calls or create remote services at runtime.

## Current bounded examples

- Product's `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` selects three catalog-read RPCs.
  Commands stay local under default composition, and additional read/identity/schema
  capabilities are not supplied by that remote runtime. See the
  [Product coverage inventory](../../crates/modules/rustok-product/docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).
- `RUSTOK_PROFILE_MEDIA_PROVIDER=grpc` selects Profiles' public-image descriptor
  capability. Image bytes still use the Media-owned HTTP/storage delivery path.
- Whole-module isolated Media/Search services are a
  [Proposed pilot](../../DECISIONS/2026-07-16-media-search-extraction-boundaries.md),
  not a universal current deployment guarantee.

## Readiness and evidence

FBA statuses apply to the stated capability/profile. `boundary_ready` can use the
central static-evidence policy; it does not certify compilation or remote deployment.
`transport_verified` is also scoped and must identify executed provider/consumer evidence;
it is not a synonym for complete whole-module extraction.

Source checks, compiled owner tests, loopback conformance, separate-process startup and
mounted business/recovery tests establish different facts. Record revision/date and actual
commands. Preserve historical results without presenting them as a current revision pass.
See the [verification guide](../backend/module-backend-verification.md#fba-evidence).

## Related Documents

- [Unified FBA Implementation Plan](./fluid-backend-architecture-unified-plan.md)
- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Fluid Frontend Architecture](./fluid-frontend-architecture.md)
- [Module Architecture](../architecture/modules.md)
- [API and Surface Contracts](../architecture/api.md)
- [Event Flow Contract](../architecture/event-flow-contract.md)
- [Module and Owner Registry](../modules/registry.md)
- [Module Authoring](../modules/module-authoring.md)
