# RBAC source of truth and staged runtime rollout

- Date: 2026-02-26
- Status: Accepted

## Context

The platform has already been migrated to a relation-derived RBAC graph as the sole source of permission data. The next migration step concerns not the data source, but the runtime decision path: the relation-resolver must follow a controlled path to `casbin_only` without losing observability and without reverting to legacy decision layers.

The key architectural goal: `crates/rustok-rbac` remains the policy host, while `apps/server` is limited to adapter/wiring responsibilities.

## Decision

1. **Source of truth for RBAC data — only the relation model.**
   - Canonical tables: `roles`, `permissions`, `user_roles`, `role_permissions`.
   - The legacy column-based role path is absent from the authorization decision.

2. **The runtime rollout is fixed as `relation_only -> casbin_shadow -> casbin_only`.**
   - `relation_only`: the relation-resolver is authoritative.
   - `casbin_shadow`: the relation-resolver is authoritative, the Casbin path runs as a parity check.
   - `casbin_only`: the authorization decision is made by the Casbin runtime.

3. **Dual-read and legacy-role paths are not a valid part of the target architecture.**
   - They are not developed further.
   - Their presence in runtime/docs/scripts is considered cleanup debt and must be removed.

4. **Module-first boundary is mandatory.**
   - `rustok-rbac` owns evaluator/runtime/shadow semantics.
   - `apps/server` owns DB/cache/logging/metrics adapters.

## Consequences

### Positive

- Data and runtime are separated cleanly and predictably.
- Cutover is observable through relation-vs-casbin parity, not through legacy fallback.
- Long-term RBAC maintenance and release gates are simplified.

### Trade-offs

- A separate parity window is needed before `casbin_only`.
- Temporary coexistence of relation decision and Casbin shadow increases the volume of observability logic until cutover is complete.

### Follow-up actions

1. Keep the migration plan synchronized with the relation/casbin-only model.
2. Remove any new legacy compatibility layers at the first safe opportunity.
3. Close the `casbin_only` gate with a separate release decision.
