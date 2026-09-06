# Events runtime module contract

## Purpose

This package adapts the Events capability to the RusToK module runtime without
creating a dependency cycle between `rustok-core` and `rustok-events`.

## Responsibility Zone

The adapter owns the `events` module identity, manifest metadata, core-module
registration, dependency declaration, and module-owned admin package
placement. It does not own event schemas, outbox persistence, Iggy transport,
or host runtime readiness.

## Integration

`EventsModule` registers as a required core module and declares `outbox` as its
runtime dependency. Canonical typed contracts are imported from
`rustok-events` by their publishers and consumers. The server composes delivery
runtime state separately because this adapter has no host runtime context.

## Verification

- `cargo check -p rustok-events-module`
- `cargo test -p rustok-events-module`
- `cargo xtask module validate events`
- `cargo xtask validate-manifest`

## Related Documentation

- [Adapter implementation plan](implementation-plan.md)
- [Canonical Events documentation](../../rustok-events/docs/README.md)
- [Canonical Events implementation plan](../../rustok-events/docs/implementation-plan.md)
- [Platform event flow](../../../docs/architecture/event-flow-contract.md)
