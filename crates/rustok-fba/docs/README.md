# `rustok-fba` Documentation

`rustok-fba` is the shared contract crate for Fluid Backend Architecture metadata.

FBA keeps module service identity stable while execution topology can be embedded,
remote, hybrid, or event-driven. This crate captures only the stable metadata and call
context shape needed by module provider/consumer registries.

Boundary rules:

- `rustok-fba` depends on `rustok-api` port primitives instead of redefining them.
- Transport implementations belong in adapter crates.
- Domain rules stay in owner modules.
- Observability implementation remains in `rustok-telemetry`; this crate may only carry
  metadata needed to describe parity requirements.

Current entry points:

- `BackendTopology`
- `TransportProfile`
- `CapabilityId`
- `FbaCallContext`
- `FbaProviderDescriptor`
- `FbaConsumerDependency`

Use this crate for provider/consumer metadata and topology descriptors. Port request,
response and error semantics still use `rustok-api::ports` primitives; transport
implementations stay in owner modules or adapter crates.

Related guide: [Backend Module Architecture](../../../docs/backend/module-backend-architecture.md).
