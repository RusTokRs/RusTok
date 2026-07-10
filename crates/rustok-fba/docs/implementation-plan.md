# rustok-fba implementation plan

## Current state

`rustok-fba` provides serializable metadata types for backend topology,
transport profiles, provider descriptors, consumer dependencies, and call
context. It reuses `rustok-api` port context, policy, and error types. No
module registry currently consumes these Rust types in production; existing
module-local FBA registries remain JSON evidence artifacts.

## Readiness

- FFA/FBA status: `not_started` — this is a metadata support crate, with no UI,
  transport implementation, or provider/consumer runtime boundary of its own.
- Owner: platform architecture.
- Boundary: `rustok-fba` may describe a boundary but must not own service
  traits, HTTP/gRPC/event adapters, runtime composition, or domain policy.
- Dependency: `rustok-api::ports` remains the canonical source of call context
  and error semantics.

## Next results

1. **Adopt typed metadata only after demonstrated repetition.** When two or
   more module registries require the same provider/consumer descriptor shape,
   introduce an owner-approved conversion from the module artifact to
   `rustok-fba` types. Done when the adoption removes duplicate metadata rather
   than creating a parallel registry path.
2. **Lock the first adopted wire contract.** Add JSON fixtures and
   serialization/backward-compatibility tests for the concrete descriptor,
   including topology, transport, degraded modes, and capability version. Done
   when a breaking metadata change fails a repeatable check.
3. **Add a registry-use guard after adoption.** Verify that new registries use
   the shared metadata shape where it applies, while retaining module-local
   domain evidence and `rustok-api` port semantics. Done when the guard rejects
   duplicate shared metadata without forcing unrelated registries to migrate.

## Verification

- `cargo test -p rustok-fba`
- Targeted registry fixture/compatibility checks once a module adopts the
  shared types.
- Review against [backend module architecture](../../../docs/backend/module-backend-architecture.md).

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Backend module implementation guide](../../../docs/backend/module-backend-implementation.md)
