# rustok-modules documentation

## Purpose

This Core module owns the module platform control plane and artifact lifecycle.

## Responsibility Zone

It owns marketplace release identity, dependencies, installation, activation,
tenant policy, rollback, capability grants and static-promotion admission. It
does not own sandbox implementation or Alloy source authoring.

## Integration

Rhai and WebAssembly artifact descriptors select executors from
`rustok-sandbox`. Alloy drafts carry source lineage and publish immutable module
releases. The server supplies infrastructure adapters and mounts owner transports.

## Verification

- `cargo xtask module validate modules`
- `cargo test -p rustok-modules`
- `cargo check -p rustok-server --lib`

## Related Documents

- [Implementation plan](./implementation-plan.md)
- [Neutral sandbox ADR](../../../DECISIONS/2026-07-11-neutral-sandbox-foundation.md)
- [Module control-plane plan](../../../docs/modules/module-control-plane-consolidation-plan.md)

