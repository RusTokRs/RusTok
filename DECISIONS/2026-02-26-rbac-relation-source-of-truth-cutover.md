# RBAC source of truth and decision engine

- Date: 2026-02-26 (amended 2026-10-25)
- Status: Accepted

## Context

The platform has migrated to a relation-derived RBAC graph as the sole source
of permission data. This decision records both that data model and the runtime
decision engine that evaluates it. An earlier revision of this document
planned a staged rollout to an external policy engine; that track was never
implemented and is removed — the implemented engine below is the target
architecture, not a migration stage.

The key architectural goal: `crates/modules/rustok-rbac` remains the policy
host, while `apps/server` is limited to adapter/wiring responsibilities.

## Decision

1. **Source of truth for RBAC data — only the relation model.**
   - Canonical tables: `roles`, `permissions`, `user_roles`, `role_permissions`.
   - The legacy column-based role path is absent from the authorization decision.
   - Dual-read and legacy-role paths are not a valid part of the target
     architecture; their presence in runtime/docs/scripts is cleanup debt.

2. **The decision engine is the platform's own set-based evaluator.**
   - The tenant-scoped relation graph is resolved to a typed permission set
     (`Resource` × `Action` in `rustok-api`).
   - `rustok-rbac::services::permission_policy` is the single canonical
     evaluator (`check_permission` / `check_any_permission` /
     `check_all_permissions`); the only wildcard rule is that a `manage`
     grant covers every action on its resource
     (`rustok_api::has_effective_permission`).
   - No external policy engine, policy-file model, or shadow/parity mode is
     part of the architecture. `AuthzEngine::Policy` names this engine in
     authorization decision telemetry.

3. **Roles are tenant database records.**
   - Every tenant is seeded with four default roles (`super_admin`, `admin`,
     `manager`, `customer`) as persisted records, seeded from bootstrap
     defaults; after seeding, persisted grants are the truth and may diverge
     from the defaults.
   - Only the super administrator role can never be deleted, and its grant
     set is immutable through the GraphQL control plane (break-glass
     protection). Every other role — defaults and custom roles alike — can
     be edited within the actor's authority ceiling and deleted while
     unassigned.

4. **Module-first boundary is mandatory.**
   - `rustok-rbac` owns evaluator/runtime semantics.
   - `apps/server` owns DB/cache/logging/metrics adapters.

## Consequences

### Positive

- Data and runtime are separated cleanly and predictably.
- One evaluator implementation: permission semantics change in exactly one
  place.
- Long-term RBAC maintenance is simplified; there is no external policy
  engine to keep in parity with the relation graph.

### Trade-offs

- Platform-specific permission semantics (the `manage` wildcard, `Own`
  scopes) are owned and tested in-tree rather than delegated to a library.

## Current owner persistence boundary (2026-09-13)

The canonical owner now reads persisted role-mutation facts and enforces active
administrator continuity inside host-supplied transactions. Target authority
and tenant system-role locks belong to `rustok-rbac`; the server supplies
authenticated actor evidence and composes user writes, commit, outbox delivery,
cache integration, and telemetry. Ordinary committed replacement shares the
same owner exact-assignment and continuity implementation as user administration.
SQLite runtime regressions are bounded evidence; PostgreSQL concurrency,
transport parity, and transaction-spanning revocation fences remain separate
completion gates.
