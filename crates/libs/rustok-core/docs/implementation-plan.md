# Implementation plan for `rustok-core`

## Current state

`rustok-core` is the minimal shared foundation layer for runtime RBAC/security
policy, typed primitives, validation, resilience, event-dispatch helpers, and
foundation utilities. It must not become a host or domain dumping ground.

Auth lifecycle belongs to `rustok-auth`; canonical `Port*`, permission, and
locale contracts belong to `rustok-api`. Core consumes those contracts where
needed and keeps no compatibility exports for their former locations.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This foundation crate has no module-owned UI or FBA provider port.

## Open results

1. **Execute the documented foundation verification suite.** Run module and
   targeted primitive/security/resilience/event-dispatch checks in an available
   compilation environment.
   **Depends on:** a build environment with the relevant workspace dependencies.
   **Done when:** current public contracts, compatibility removals, dispatcher
   retry/backpressure behavior, and latency metric hooks have compiled evidence.

2. **Extend foundation behavior only for real cross-module need.** Add a shared
   primitive or helper only when at least two independent consumers need the
   same contract; otherwise keep it with the owning module.
   **Depends on:** demonstrated cross-module usage and an owner decision.
   **Done when:** the public surface, consumer migration, and targeted contract
   tests prove the addition does not reintroduce domain ownership.

3. **Maintain foundation contract discipline.** Synchronize public API, local
   docs, module metadata, and consumer docs with a changed core behavior.
   **Depends on:** the change-owning foundation contract.
   **Done when:** no removed auth/API/locale/port export or domain dependency
   returns through compatibility code.

## Verification

- Contract tests cover every public use case.
- `cargo test -p rustok-core`
- `cargo check -p rustok-core`
- Targeted primitives, validation, security, RT JSON sanitization, cache/
  resilience, event observability, dispatcher retry/backpressure, and public
  compatibility tests.

## Change rules

1. Keep domain behavior in its owner module and shared contracts in their
   canonical foundation crate.
2. Update the root README and local docs with a core contract change.
3. Update consumer docs whenever a shared behavior changes live semantics.

## Acknowledged sealed module delivery (2026-11-01; source only)

The real Inventory/Search continuation extends existing EventHandler, dispatcher, bus and Memory
with typed opt-in/awaited errors, bounded concurrency and Weak lifecycle binding. Root APIs and
best-effort broadcast remain unchanged; retry/DLQ stays Outbox-owned, not a new Core queue.
Server startup now propagates listener binding failures. Six focused regression definitions are
**uncompiled/unrun**. Remaining: scoped Rust compilation, lifecycle/capacity/shutdown races and
configured delivery/restart evidence. See the [API](../CRATE_API.md#acknowledged-sealed-module-delivery-2026-11-01-source-only)
and [decision](../../../../DECISIONS/2026-07-17-sealed-typed-event-families.md#acknowledged-local-contract-listeners-2026-11-01).
