# Cross-module owner diagnostic ports

- Date: 2026-09-29
- Decision status: Accepted
- Implementation status: Implemented via PR #4333, squash merge `9bf5c48a18f6b27f54f86ed4e878ef43193bb31d`.
- Owners: Forum and Notifications module owners
- Extends: [Port contract ownership and runtime feature boundary](./2026-07-01-port-contract-ownership-and-runtime-feature-boundary.md), [ModuleRuntimeExtensions for runtime capabilities](./2026-04-20-module-runtime-extensions-for-capabilities.md)
- Supersedes: None
- Superseded by: None

## Context

Optional modules sometimes need operator diagnostics that combine a policy-owned transport with
state owned by another module. A server-owned GraphQL shim creates the wrong dependency direction:
`apps/server` begins to own module-specific DTOs, RBAC policy, and owner-service construction even
though the affected modules already have manifest-owned GraphQL query roots and typed runtime
capabilities.

The Forum notification reconciliation diagnostic is the concrete case. Forum owns the operator
authorization and GraphQL surface. Notifications owns notification persistence, current recipient
privacy/source authorization, reconciliation inspection, and the repair path. Neither owner should
move its domain responsibility into `apps/server`, and Notifications must not acquire Forum-specific
GraphQL/RBAC knowledge merely because its data is being inspected.

## Decision

Cross-module read-only diagnostics must use a neutral typed owner port:

1. The consuming domain module owns its operator GraphQL field, DTO, tenant/RBAC admission, and
   transport telemetry.
2. The state-owning module exposes a narrow read port containing only the evidence needed by the
   consumer; it does not expose persistence models or require consumers to instantiate owner
   services.
3. The port uses `rustok_api::PortContext` for tenant, actor, correlation, deadline and claims.
   Tenant identity is not duplicated in the request DTO.
4. When construction needs DB-backed host capabilities, the owner registers a deferred port factory
   through `ModuleRuntimeExtensions` and materializes it from `HostRuntimeContext`.
5. Missing owner capabilities fail closed. There is no direct table fallback and no server-specific
   shim.
6. The executable server composes manifest-declared module query roots but does not know the
   cross-module diagnostic's implementation types.

## Sources of truth and ownership

- Forum owns operator authorization, the `forumNotificationReconciliationStatus` GraphQL field,
  its response mapping, and transport observability.
- Notifications owns notification inbox state, current open-time privacy/source authorization,
  `NotificationInboxReconcileService`, and the neutral reconciliation inspection port.
- `rustok-notifications-api` owns only the transport-neutral request/result/port/factory contracts.
- `apps/server` owns runtime composition and transfers typed capability values; it owns no
  Forum- or Notifications-specific reconciliation behavior.
- Notifications persistence and the existing `reconcile_page` state transition remain the sole
  repair authority.

## Invariants

### Allowed states

- Forum may use the Notifications inspection port when the Notifications capability is composed.
- A bounded exact-recipient page may be inspected without mutation.
- A page may contain unavailable rows; `clean` is page-local evidence only.
- The next page is reached only through the owner-provided bounded cursor.

### Forbidden states

- `apps/server` must not define a module-specific reconciliation GraphQL shim.
- Forum must not read Notifications private tables or instantiate
  `NotificationInboxReconcileService`.
- Notifications must not contain Forum-specific operator permissions or GraphQL policy.
- The inspection path must not archive, mark read/unread, create delivery attempts, or schedule
  retries.
- Cross-tenant recipient access must be impossible through the port context.

## Non-goals

This decision does not define repair authorization, scheduled tenant-wide reconciliation,
delivery-time authorization, persistence schema changes, or cross-module write transactions.

## Data, transaction, and concurrency boundary

The diagnostic is read-only. The Notifications inspection implementation scans one bounded
tenant/recipient page and invokes current open-time authorization outside any long-lived write
transaction. No new durable state is introduced. The existing destructive `reconcile_page` path
and exact inbox state owner retain their current idempotent semantics.

## Context dimensions

Tenant identity is carried by `PortContext.tenant_id`. The consuming Forum resolver binds it to the
trusted `TenantContext` and passes the authenticated operator actor, locale, correlation id,
claims, and a finite read deadline. The request carries only the recipient UUID and bounded cursor
parameters.

## Events and projections

No event, outbox record, projection, or cache is introduced. The diagnostic is current-policy
evidence and must not be treated as a serializable repair fence.

## Failure semantics

Invalid request context is rejected as a client error. Missing runtime capabilities, timeout,
database, privacy, or source-authority failures are mapped to a stable unavailable/internal
diagnostic response. No partial clean result is returned after a retryable owner failure.

## Migration and cutover

The existing server-owned reconciliation shim is removed. The GraphQL field remains available
through the Forum manifest-owned query root. No data migration is required.

## Alternatives considered

- Keep the server-owned GraphQL shim: rejected because it violates the composition-root/module-owner
  boundary and makes the server depend on module implementation types.
- Move the Forum field into Notifications GraphQL: rejected because Notifications would then own
  Forum-specific RBAC and transport semantics.
- Copy Notifications rows or reconciliation logic into Forum: rejected because it creates a second
  state/reconciliation authority and bypasses current Notifications policy checks.

## Verification

Required evidence includes:

- static source guard proving no server-owned reconciliation shim exists;
- source verification proving Forum GraphQL uses only the neutral Notifications port;
- Notifications source verification proving the port delegates to the existing bounded inspection
  pipeline and exposes no mutation;
- module runtime registration evidence for the deferred factory;
- maintainer compile/test execution after merge.

## Consequences

The platform keeps domain-specific transport policy in the owning module while preserving a thin,
typed cross-module capability boundary. The additional API contract and deferred factory add a
small amount of boilerplate, but prevent direct owner coupling and make future cross-module
diagnostics follow the same runtime-capability pattern.
