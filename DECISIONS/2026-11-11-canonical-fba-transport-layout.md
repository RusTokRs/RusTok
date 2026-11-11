# Canonical FBA transport layout

- Date: 2026-11-11
- Decision status: Superseded
- Implementation status: In progress
- Owners: platform architecture / native module owners / xtask
- Extends: [Canonical native module source layout](./2026-09-18-canonical-native-module-source-layout.md), [Port contract ownership](./2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md)
- Supersedes: The former separate transport and service placement rule for native FBA modules
- Superseded by: [Canonical FBA owner ports and deployment](./2026-11-11-canonical-fba-owner-ports.md)

## Context

Native modules use one owner directory. Product's existing standalone profile put
the gRPC adapter and its composition root in two sibling packages. The different
paths obscure the owner boundary during scaffold, conversion, and review.

## Decision

New and converted native FBA owners follow the same
[remote FBA module layout](../docs/backend/module-backend-implementation.md#remote-fba-module-layout).
An owner with a real remotely served port has one nested Cargo package under
`transport/`. Its library adapts owner ports to gRPC. Its `src/bin/service.rs`
composes the standalone process and is built with the `service` feature.
The owner has one module identity and an explicit dependency from adapter to owner.

Embedded-only modules use the native owner scaffold. The scaffold documents the
same extension point that existing modules use when they add a real remote port.
The transport package joins Cargo workspace members without receiving a new
`modules.toml` entry. The host selects embedded or gRPC execution for each supported
capability at startup. Cargo features select build targets.

## Sources of truth and ownership

`modules.toml` owns module identity. The domain crate owns the public ports and
their implementation. The transport owns the RPC schema, framing and network
admission. The service entry point composes configuration, providers, telemetry
and shutdown.

## Invariants

One `transport/` package belongs to one native owner. The owner can build
independently. The standalone binary is opt-in. The host selects the deployment
profile for each supported capability.

## Non-goals

This decision covers source and build placement. Capability coverage, database
separation and rollout readiness follow their own owner contracts. FFA keeps its
UI-specific layout and transport contracts.

## Data, transaction, and concurrency boundary

The owner retains data authority, migrations and transaction rules in both
profiles. A capability's deployment plan defines placement and handles any
cross-process consistency requirement.

## Context dimensions

Both profiles preserve owner-defined tenant, actor, locale, channel and policy
semantics. The gRPC adapter maps authenticated context into the same owner port.

## Events and projections

The owner writes domain events and owns projection triggers. The service composes
the owner's existing outbox or delivery dependencies as required by its profile.

## Failure semantics

Provider startup and unavailable calls fail according to the configured profile.
Ambiguous write outcomes, retries and idempotency follow the owner contract.

## Migration and cutover

Product and Media have existing sibling packages. Their placement is a migration
state for those packages. Preserve published service identity, schema and operation
semantics during conversion. Update active Cargo references, launch commands and
checks together, and keep historical execution evidence attached to its revision.

## Alternatives considered

An all-in-one Cargo package would move protocol dependencies and code generation
into the owner build. Separate sibling adapter and service packages preserve build
boundaries while scattering one owner's implementation. The nested package keeps
the owner independent of its wire protocol and groups the wire code with startup.

## Verification

Use `cargo xtask module lint <slug> --fba` for the packaging gate and verify the
owner library, adapter library and `service` binary in their respective build
profiles. Verify real callers and owner operations through both transports before
promoting readiness. Additive generation of an owner-specific adapter requires the
real port and protocol implementation.

## Consequences

New module scaffolding and existing module conversion use one placement contract
and one packaging check. Existing deployments adopt it when their packages move.
