# rustok-fba

## Purpose

`rustok-fba` owns Fluid Backend Architecture contracts for backend module boundaries.

## Responsibilities

- Describe backend topology and transport profiles without owning the transport itself.
- Provide shared provider/consumer metadata types for FBA registries.
- Reuse `rustok-api::ports` primitives for call context and errors.

## Entry Points

- `BackendTopology`
- `TransportProfile`
- `CapabilityId`
- `FbaCallContext`
- `FbaProviderDescriptor`
- `FbaConsumerDependency`

## Interactions

- Depends on `rustok-api` port contracts.
- Used by module FBA registries, provider descriptors, and future generated runtime
  registries.
- Does not own gRPC/HTTP implementations, domain services, CLI, runtime composition, or UI.

See [docs](docs/README.md).

