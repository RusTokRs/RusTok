# rustok-fba

## Purpose

`rustok-fba` owns Fluid Backend Architecture contracts for backend module boundaries.

## Responsibilities

- Describe backend topology and transport profiles without owning the transport itself.
- Provide shared provider/consumer metadata types for FBA registries.
- Own the canonical v1 provider-registry structural schema used by fast verifier gates.
- Reuse `rustok-api::ports` primitives for call context and errors.

## Entry Points

- `BackendTopology`
- `TransportProfile`
- `CapabilityId`
- `FbaCallContext`
- `FbaProviderDescriptor`
- `FbaConsumerDependency`
- `contracts/provider-registry-v1.schema.json`

## Interactions

- Depends on `rustok-api` port contracts.
- Used by module FBA registries, provider descriptors, and future generated runtime
  registries.
- Does not own gRPC/HTTP implementations, domain services, CLI, runtime composition, or UI.
See [CRATE_API.md](CRATE_API.md) for the formal public API surface and contract invariants.

See [docs](docs/README.md).

## Metadata and readiness limits

These descriptors are declarative. They do not implement discovery, provider selection,
capability negotiation, dependency enforcement, fallback or service lifecycle. Composition
roots and owner adapters implement actual execution. A capability/profile may cover only
part of a module; its status is not complete whole-module remote certification.
Owner ports/DTOs may remain in their implementation crate or an optional owner contracts
package. See the [FBA contract](../../../docs/backend/module-backend-architecture.md#fba-contracts).
