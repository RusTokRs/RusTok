# Canonical FBA owner ports and deployment

- Date: 2026-11-11
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform architecture / native module owners / xtask
- Extends: [Port contract ownership](./2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md), [Native module source layout](./2026-09-18-canonical-native-module-source-layout.md)
- Supersedes: [Canonical FBA transport layout](./2026-11-11-canonical-fba-transport-layout.md), optional placement of published native owner contracts, and attached-value authority in [Taxonomy and Flex ownership](./2026-08-22-taxonomy-category-flex-ownership.md) as scoped below
- Superseded by: None

## Context

Native domain owners need one structure for new modules and for conversion of existing
modules. A transport package that imports the whole owner makes remote consumers link
its implementation. Mutual business calls can also become cyclic Cargo dependencies
when both packages import each other's runtimes, including through optional features.

## Decision

Each native domain or capability owner is designed for embedded and independent
execution. The modular monolith is the default deployment. Operators choose which
capabilities and owners to move to separate processes. A remote caller can use the
same gRPC adapter hosted by the monolith or by the standalone owner process.

Use one owner directory and one module identity. The root Cargo package owns the
implementation, migrations and embedded registration. Published owner ports, DTOs,
errors and event contracts live in `ports/`, a workspace package named
`<owner-package>-ports`. It can build without owner runtimes or storage drivers.
The root depends on its ports package. Real request/response profiles publish one
wire schema and optional `grpc-client` and `grpc-server` adapters in that package.
The standalone composition root is the owner's `src/bin/service.rs` binary, enabled
by `service`. Hosts select local or gRPC implementations at startup. Cargo features
add code to a build and are independent of runtime provider selection.

The [implementation guide](../docs/backend/module-backend-implementation.md#remote-fba-module-layout)
defines the shared layout and its verification. Introduce source only for actual
operations. An event-only profile uses the owner event contract and delivery
adapter. Platform libraries and host applications have their own composition roles.

## Sources of truth and ownership

Cross-owner calls use the provider's public ports. Publish owner facts through the
canonical event/outbox contract. The consumer receives explicit provider bindings.
Composition roots can link implementations for the monolith. Owner runtimes consume
contracts, while the standalone process assembles its owner and configured outgoing
adapters without linking the monolith, module control plane or other owner runtimes.

The owner of a schema, policy or extension rule owns its definition and lifecycle.
An aggregate owner owns its attached values, writes and migrations. Commit those
values with the aggregate when the operation requires atomicity. For example, Flex
owns field schemas, policies and standalone entries. Product owns attached values
on products, and Taxonomy owns attached values on categories. The same rule applies
to other extension capabilities and donors. A capability edits a donor through the
donor's port, and a donor obtains required definitions or decisions through the
capability's port or its explicitly versioned projection.

This supersedes authority over donor-attached values and cross-owner shared-storage
requirements in the Taxonomy/Flex decision. Existing storage, policies, Translation
projections, revisions and change delivery remain live migration responsibilities.
Preserve their contracts while moving authority. Flex remains a capability owner
even while embedded; its standalone schemas/entries and process placement are distinct.

## Invariants

- One owner identity and one canonical implementation serve every supported profile.
- Consumers select contracts and receive providers through host composition.
- Private SQL, repositories, concrete owner services and transaction handles do
  not cross the public boundary.
- The server adapter can be mounted in either host without changing consumer code.

## Non-goals

This decision does not deploy every module as a service or certify existing
profiles. It leaves FFA and the separately governed WASI guest model intact.

## Data, transaction, and concurrency boundary

A published port has transport-neutral inputs and outputs. Keep ORM entities, private
SQL, database transactions and concrete services inside the relevant runtime. Existing
cross-owner transaction participants remain co-located until the owning operation
defines and verifies its replacement consistency, retry and recovery guarantees.
An adapter alone does not replace an atomic write across owners.

## Context dimensions

Both providers enforce the same owner policy, tenant, actor, channel and locale
semantics. The network adapter authenticates callers and maps trusted context into
the owner port. Carry deadlines and trace context across the transport boundary.

## Events and projections

Owner mutations and their outbox records commit together. Delivery and derived
projections have explicit replay, deduplication, recovery and lag semantics.

## Failure semantics

Required unavailable capabilities fail explicitly. Use owner idempotency and
recovery for uncertain write outcomes. An explicitly selected provider remains
authoritative when it fails.

## Migration and cutover

The [unified FBA plan](../docs/research/fluid-backend-architecture-unified-plan.md)
sequences migration by real capability. Preserve published wire identity and retained
execution evidence while changing source placement. Product and Media's current
transport packages and existing shared-storage workflows are migration states.

## Alternatives considered

Keeping all code in a single Cargo package reduces manifests but cannot express
mutual dependencies on two packages that also contain the implementations.
A separate runtime-dependent transport package isolates code generation but retains
the consumer's owner-runtime dependency. The nested ports package creates a build
boundary while keeping code, versioning and module ownership together.

## Verification

Use `cargo xtask module lint <slug> --fba` for structural checks and inspect the
resolved dependency graphs for contract, client, embedded and standalone builds.
Run shared operation cases in-process and through real remote entrypoints, including
consumer wiring, authorization, unavailable dependencies and write/event recovery
where applicable. Readiness applies to the verified scope. The accepted target and
a green layout check do not certify a module's deployment.

## Consequences

Published contracts gain a dedicated build boundary. Owners retain one registry
identity, directory and version lifecycle. Existing modules converge operation
by operation. Deployment can remain embedded or adopt supported remote profiles.
