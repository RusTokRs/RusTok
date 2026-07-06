# `rustok-telemetry` Documentation

`rustok-telemetry` — observability foundation module of the platform. It holds
shared telemetry primitives and wiring contracts that should be used uniformly
by modules and the host layer.

## Purpose

- publish the canonical telemetry/observability foundation contract;
- keep shared telemetry helpers and wiring expectations outside `apps/server`;
- reduce drift in metrics, traces and logging conventions between modules.

## Scope

- shared telemetry primitives and instrumentation helpers;
- basic observability contracts for metrics, tracing and related runtime wiring;
- foundation surface for consumer modules and host integrations;
- no domain-owned metrics semantics or transport/business logic.

## Integration

- used by `apps/server` and runtime modules as a shared observability dependency;
- module-specific metrics remain inside owning modules but are built over the common foundation contracts;
- any changes to shared telemetry wiring must be synchronized with host docs and verification docs;
- `rustok-telemetry` must not absorb domain-specific observability runbooks.

## Verification

- `cargo xtask module validate telemetry`
- `cargo xtask module test telemetry`
- targeted tests for telemetry helpers, wiring contracts and compatibility expectations

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
