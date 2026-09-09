# rustok-fba implementation plan

## Current state

`rustok-fba` provides serializable metadata types for backend topology,
transport profiles, provider descriptors, consumer dependencies, and call
context. It also owns the v1 provider-registry structural schema. The shared
fast validator adopts that schema for compatible RBAC and Workflow registry
artifacts; module-local verifiers retain their owner-specific semantics. No
module registry currently consumes these Rust types in production.

## Readiness

- FFA/FBA status: `not_started` — this is a metadata support crate, with no UI,
  transport implementation, or provider/consumer runtime boundary of its own.
- Owner: platform architecture.
- Boundary: `rustok-fba` may describe a boundary but must not own service
  traits, HTTP/gRPC/event adapters, runtime composition, or domain policy.
- Dependency: `rustok-api::ports` remains the canonical source of call context
  and error semantics.

## Next results

1. **Extend adoption only across compatible shapes.** Migrate another provider
   registry when it has the v1 required owner, port, consumer, evidence, and
   contract-test metadata. Do not force mixed-generation registries through an
   adapter or compatibility shape.
2. **Lock the first adopted wire contract.** Add JSON fixtures and
   serialization/backward-compatibility tests for the concrete descriptor,
   including topology, transport, degraded modes, and capability version. Done
   when a breaking metadata change fails a repeatable check.
3. **Keep the registry-use guard structural.** The shared validator must reject
   malformed common metadata while module gates continue to prove domain
   semantics, source order, and runtime evidence.

## Verification

- `cargo test -p rustok-fba`
- `node scripts/verify/lib/fba-registry-validation.test.mjs`
- Targeted registry fixture/compatibility checks once a module adopts the
  shared types.
- Review against [backend module architecture](../../../docs/backend/module-backend-architecture.md).

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Backend module implementation guide](../../../docs/backend/module-backend-implementation.md)
