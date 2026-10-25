# rustok-fba / CRATE_API

## Role
Fluid Backend Architecture contracts defining neutral module boundaries, capability descriptors, topology models, and transport profiles.

## Public modules
- Root crate definitions: `BackendTopology`, `TransportProfile`, `CapabilityId`, `FbaCallContext`, `FbaProviderDescriptor`, `FbaConsumerDependency`.

## Primary public types and functions
- `pub enum BackendTopology`: `Embedded`, `Remote`, `Hybrid`, `AsyncCompanion`.
- `pub enum TransportProfile`: `InProcess`, `Grpc`, `Http`, `Event`.
- `pub struct CapabilityId`: Module identifier, capability name, and semantic version.
- `pub struct FbaCallContext`: Combines `PortContext` and `PortCallPolicy` for cross-boundary invocations.
- `pub struct FbaProviderDescriptor`: Defines provided capability, topology, supported transports, and degraded modes.
- `pub struct FbaConsumerDependency`: Declares required or optional capability dependencies with fallback modes.
- `pub type FbaResult<T> = Result<T, PortError>`

## Contract invariants
- **Transport neutrality**: Contracts never own network sockets, connection pools, or protocol serializers directly.
- **Topology independence**: Provider descriptors declare capabilities orthogonally to host deployment mode.
- **Fail-safe fallback**: Consumer dependencies explicitly declare fallback modes (`fallback_modes`) when optional dependencies degrade.

## Errors
- Reuses canonical [`PortError`](rustok_api::PortError) for boundary crossing failures, timeouts, and capability unreachability.
