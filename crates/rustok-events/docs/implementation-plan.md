# Implementation plan for `rustok-events`

## Current state

`rustok-events` is the canonical source of `DomainEvent`, `EventEnvelope`,
schema metadata, validation, and event versioning policy. `rustok-core::events`
is a compatibility re-export only; domain, outbox, runtime, and test crates
should import event contracts from this module.

Schema validation and JSON roundtrip coverage exist for the public event
surface, including tenant lifecycle events. Transport-specific delivery logic
does not belong here.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This module publishes shared contracts, not a UI or FBA provider port.

## Open results

1. **Keep event types, schema registry, and consumer imports synchronized.**
   Add an event family only with canonical schema/validation coverage and direct
   consumer imports from `rustok-events`.
   **Depends on:** the change-owning domain module and its outbox path.
   **Done when:** the event, registry metadata, consumer imports, and contract
   tests describe the same payload and tenant behavior.

2. **Make schema-change release discipline executable.** Document and enforce
   version bump, compatibility, deprecation, and dual-read/migration decisions
   for breaking payload changes; continue removing residual compatibility imports.
   **Depends on:** the affected event consumer release plan.
   **Done when:** a breaking change has an explicit migration owner, supported
   reader versions, and a testable compatibility strategy.

3. **Synchronize event contracts with recovery guidance.** Update outbox,
   replay, reindex, and DLQ documentation with a schema or versioning change.
   **Depends on:** the relevant runtime/operational contract.
   **Done when:** recovery procedures name the correct event schema and do not
   rely on transport-owned copies of event payloads.

## Verification

- `cargo xtask module validate events`
- `cargo xtask module test events`
- Targeted schema coverage, validation, compatibility-alias, and envelope JSON
  roundtrip tests.

## Change rules

1. Keep canonical event payloads and schemas in this module.
2. Update local docs, `rustok-module.toml`, event-flow documentation, and
   outbox/replay guidance with a contract change.
3. Update `docs/modules/registry.md` if ownership or module status changes.
