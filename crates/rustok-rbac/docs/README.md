# `rustok-rbac` Documentation

`rustok-rbac` is the canonical RBAC runtime module in RusToK. Local
documentation for this module must live inside the crate, not spread across
`docs/architecture/*` or server-only notes.

## Purpose

- publish a unified RBAC runtime contract for permission resolution and checking;
- keep permission policy/evaluator and integration event contracts inside the module;
- keep `apps/server` in the adapter/wiring layer role, not as a second RBAC runtime.

## Scope

- relation-based source of truth: `roles`, `permissions`, `user_roles`, `role_permissions`;
- `PermissionResolver`, `RuntimePermissionResolver`, policy/evaluator and tenant policy authorization flow;
- cross-module event contracts for role assignment changes;
- permission-aware runtime contracts and typed RBAC primitives in conjunction with `rustok-core`;
- absence of rollout-mode and shadow-runtime logic in the live surface.

## Integration

- `apps/server` owns only the adapter/wiring layer: store adapters, cache integration, transport extractors and observability;
- GraphQL role query/mutation/types live in `rustok-rbac`; `apps/server` only composes roots and passes adapter role records to runtime persistence;
- `rustok-core` remains the owner of typed primitives (`Permission`, `Resource`, `Action`, `SecurityContext`);
- live authorization goes only through tenant policy evaluation, without a relation-only/shadow parity path;
- `RbacPermissionDecisionPort` resolves its tenant/user decision through the
  authoritative `PermissionResolver`; request claims are not used as an
  independent permission source;
- `RbacArtifactPermissionCatalog` is the durable owner adapter for immutable
  artifact permission vocabulary. It stores localized labels/descriptions by
  scope and admitted installation identity, is idempotent for retries, and
  never writes `roles` or `role_permissions` during registration. Its owner
  migration is aggregated by `rustok-migrations::Migrator`, the installer and
  CLI schema path used by production hosts;
- `RbacArtifactPermissionAssignmentService` owns explicit, idempotent
  tenant-role grants and revocations for that vocabulary in
  `rbac_artifact_role_permissions`; it validates the exact installation and
  platform-or-tenant catalog scope before writing, records the acting operator
  in its durable operation ledger, and never mutates static `role_permissions`.
  `SeaOrmArtifactPermissionAuthorizer` resolves the matching role-derived grant
  for an exact tenant, user, installation, and permission key. The platform
  artifact HTTP and command routes are runtime consumers; they never interpret
  a module-defined permission as a static `Permission` enum value;
- the operator-facing admin overview lives in `rustok-rbac-admin` and is structured as FFA `core` + native-only `transport` + `ui/leptos` adapter;
- new public RBAC surfaces and event contracts require synchronization of module docs, server docs and verification plan.

## Observability and release gates

Canonical authorization signals:

- `rustok_rbac_permission_cache_hits`
- `rustok_rbac_permission_cache_misses`
- `rustok_rbac_permission_checks_allowed`
- `rustok_rbac_permission_checks_denied`
- `rustok_rbac_claim_role_mismatch_total`
- `rustok_rbac_engine_decisions_policy_total`
- `rustok_rbac_engine_eval_duration_ms_total`
- `rustok_rbac_engine_eval_duration_samples`

Canonical durable-invalidation signals:

- `rustok_rbac_invalidation_durable_generation` — database source-of-truth generation;
- `rustok_rbac_invalidation_applied_generation` — generation applied to this process;
- `rustok_rbac_invalidation_generation_lag` — signed durable minus applied generation; a negative value is a database regression;
- `rustok_rbac_invalidation_watchdog_running` — `1` while the supervised watchdog worker is running;
- `rustok_rbac_invalidation_watchdog_restarts_total{reason}` — bounded reasons `panic`, `unexpected_exit`, or `runtime_replaced`;
- `rustok_rbac_invalidation_database_read_errors_total` — durable-generation read failures after installation;
- `rustok_rbac_invalidation_recoveries_total{reason}` — bounded reasons `initial_sync`, `generation_advanced`, or `generation_regressed`;
- `rustok_rbac_invalidation_full_clears_total{reason}` — process-wide permission snapshot clears caused by the same bounded recovery reasons.

The invalidation metrics deliberately have no tenant, user, role, permission, session,
client, or cache-key labels. They describe process-level recovery state and must remain
bounded across replicas.

Release gates for changes in the module:

- update unit tests for changed domain logic;
- verify compatibility with server adapters;
- synchronize `README.md`, local docs and verification docs;
- do not reintroduce rollout-mode or a second live authorization path.

## Durable invalidation alert policy

The watchdog reconciles every five seconds. Operators should apply these baseline
thresholds and tune only after retaining workload evidence:

- warning when `rustok_rbac_invalidation_generation_lag > 0` for two consecutive
  polls (10 seconds); critical when it remains positive for 30 seconds;
- immediate critical when `rustok_rbac_invalidation_generation_lag < 0`, because
  the database generation has regressed below the process checkpoint;
- critical when `rustok_rbac_invalidation_watchdog_running == 0` for more than two
  seconds outside a controlled shutdown;
- warning when watchdog restarts increase by at least three in ten minutes;
  critical at ten in ten minutes or any sustained restart loop;
- critical after three consecutive durable-generation read failures or 15 seconds
  without a successful read after the generation table is installed;
- investigate every `generation_regressed` recovery or full clear. An
  `initial_sync` full clear is expected once per process start, while repeated
  `generation_advanced` clears indicate missed fast-path invalidations or listener lag.

## Durable invalidation incident runbook

### Redis outage or restart

1. Confirm authorization writes still commit and advance
   `rustok_rbac_invalidation_durable_generation`; Redis is not the authority.
2. Confirm the mutating replica applies the committed generation immediately.
3. Watch every other replica. Its positive generation lag must return to zero within
   two watchdog intervals under normal database availability.
4. If lag persists, inspect the watchdog-running gauge and database-read error
   counter before restarting anything. Restore database connectivity or the worker;
   do not reset the durable generation or introduce a Redis counter.
5. Verify a recovery and full-clear increment with reason `generation_advanced`,
   then recheck representative allowed and denied permissions through an approved
   authenticated operator path.

### Missed PubSub event

1. Record the durable and applied generation values and the affected replica.
2. Confirm positive lag is visible before reconciliation.
3. Confirm the watchdog clears process permission snapshots, advances the applied
   generation, increments `generation_advanced`, and returns lag to zero.
4. If the applied generation does not advance, treat the replica as unsafe for
   authorization traffic until the database read or watchdog failure is corrected.

### Generation regression

1. Treat any negative lag as a release-blocking database restore or topology error.
2. Freeze role and permission mutations while comparing the authoritative database,
   replicas and restore point. Do not lower an in-process checkpoint to match the
   regressed database value.
3. Confirm the watchdog records `generation_regressed` and clears all process
   permission snapshots.
4. Restore monotonic database state through the owned migration/repair path and
   verify every replica returns to zero lag before reopening control-plane writes.

### Canonical role repair

1. Produce the read-only repair plan first and review the affected system roles.
2. Execute the approved `rbac repair-system-roles --apply` command against the
   authoritative database.
3. Confirm the repair transaction commits one new durable generation.
4. Confirm local invalidation occurs immediately and remote replicas recover to the
   committed generation without a restart.
5. Verify representative allow and deny decisions. Do not infer success only from
   the CLI exit code or Redis publication.

The current metrics and runbook make generation recovery observable, but a complete
single-incident chain from evaluator decision through relation state, cache snapshot,
durable generation and recovery action is still an open P1 in the implementation plan.

## Verification

- `cargo xtask module validate rbac`
- `cargo xtask module test rbac`
- `cargo test -p rustok-telemetry rbac_invalidation_metrics`
- `cargo test -p rustok-server --lib rbac_invalidation_generation`
- `node scripts/verify/verify-rbac-invalidation-observability.mjs`
- targeted tests for permission resolution, tenant policy decisions and integration events

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
- [Verification plan](../../../docs/verification/rbac-server-modules-verification-plan.md)
