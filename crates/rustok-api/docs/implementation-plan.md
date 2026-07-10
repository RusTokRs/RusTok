# Implementation plan for `rustok-api`

## Current state

`rustok-api` owns neutral shared host/API contracts: request, auth, tenant,
channel, GraphQL, route, locale, permission, and transport-agnostic port
primitives. `PortContext`, `PortError`, and `PortCallPolicy` provide shared
read/write/replay/best-effort policy without module business logic.

The crate has no dependency on `rustok-core`; core owns runtime RBAC/security
and consumes API contracts. `apps/server` is the composition root, not a second
shared API framework. Module resolvers, controllers, and domain ports remain
with their owners.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This shared-contract crate has no module-owned UI or FBA provider port.

## Open results

1. **Keep shared contract extraction evidence-based.** Move a helper into this
   crate only when it is framework-neutral and needed by independent consumers;
   keep module resolvers, controllers, and domain policy with their owners.
   **Depends on:** demonstrated multi-module use and owner approval.
   **Done when:** the shared API is dependency-neutral, consumers remove local
   duplicates, and no domain behavior enters the crate.

2. **Preserve port-policy consistency across consumers.** Evolve `PortContext`,
   `PortError`, and `PortCallPolicy` atomically for read, write, replay, and
   best-effort semantics.
   **Depends on:** all registered port consumers and their public contracts.
   **Done when:** targeted migration tests prove identical deadline, idempotency,
   actor, and typed-error behavior without local policy forks.

3. **Maintain composition and documentation boundaries.** Update API docs,
   server composition docs, and module transport docs with a changed shared
   contract, and run the focused surface verification.
   **Depends on:** the changed public contract.
   **Done when:** the documentation and `verify:api:surface-contract` describe
   the same dependency direction and owner responsibilities.

## Verification

- `npm run verify:api:surface-contract`
- Targeted compile/tests when changing shared request, auth, tenant, channel,
  GraphQL, route, locale, permission, or port contracts.
- Documentation synchronization for `apps/server` and module-owned transports.

## Change rules

1. Keep this crate neutral and dependency-light; do not add module business
   logic, resolver ownership, or runtime composition.
2. Update the root README and local docs with a public contract change.
3. Update host and consumer-module documentation with changed shared semantics.
