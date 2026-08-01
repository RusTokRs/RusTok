# RBAC durable invalidation operations runbook

This runbook covers authorization-cache recovery driven by the PostgreSQL-backed
RBAC invalidation generation. Redis and process-local publication are latency
optimizations; the durable database generation remains the recovery authority.

## Signals

Use the following bounded Prometheus metrics together with structured logs:

- `rustok_rbac_invalidation_database_generation` — generation read from PostgreSQL;
- `rustok_rbac_invalidation_applied_generation` — highest generation applied by the process;
- `rustok_rbac_invalidation_generation_lag` — database generation minus process generation;
- `rustok_rbac_invalidation_worker_running{worker="durable_generation_watchdog"}` — watchdog liveness;
- `rustok_rbac_invalidation_worker_restarts_total` — panic or unexpected-exit restarts;
- `rustok_rbac_invalidation_recoveries_total` — successful catch-up or regression recovery actions;
- `rustok_rbac_invalidation_full_clears_total` — fail-safe full permission-snapshot clears.

Suggested initial alerts:

- page when generation lag remains above zero for more than two reconciliation intervals;
- page when the watchdog running gauge is zero for more than one restart delay;
- warn on any generation regression or repeated watchdog restart;
- warn when database-generation reads fail continuously for more than one minute.

Tune thresholds only after retained multi-replica evidence establishes the normal
reconciliation distribution. Never silence sustained lag by increasing the bound
without proving that stale authorization snapshots cannot survive longer.

## Redis outage or missed publication

1. Confirm PostgreSQL is reachable and the database generation continues to advance after a committed role or permission mutation.
2. Confirm each replica reports a running durable-generation watchdog.
3. Compare database and applied generation on every replica. A lagging replica must clear all permission snapshots and converge within the watchdog interval.
4. Restore Redis without restarting the server. Redis resubscription may clear snapshots again; this is safe.
5. Exercise one deny-to-allow and one allow-to-deny permission transition against every replica. Verify the resulting evaluator decision and applied generation.
6. Retain the metric interval, correlated logs, mutation identity and effective-permission probe as incident evidence.

Do not treat successful Redis publication as authorization correctness evidence.
The database generation and replica convergence are authoritative.

## Generation regression

A database generation lower than the process-applied generation indicates restore,
manual modification or storage corruption. The watchdog clears all permission
snapshots and emits an error, but it does not lower its monotonic process checkpoint.

1. Freeze RBAC mutations and identify the database restore or write that caused the regression.
2. Compare the singleton `rbac_invalidation_state` row with the latest committed authorization mutations and backup lineage.
3. Restore a generation that is at least the highest generation observed by any live replica. Do not reset it to zero.
4. Restart or replace replicas only after the durable generation is repaired. Confirm zero lag and a fresh full clear on every replica.
5. Re-run representative effective-permission checks before unfreezing mutations.

## System-role repair while replicas are live

1. Run the read-only repair plan first and retain the proposed relation changes.
2. Execute `rbac repair-system-roles --apply` only with the approved operator identity.
3. Confirm the repair transaction reports one committed durable generation.
4. Confirm every replica reaches that generation without restart and records a recovery or direct application.
5. Validate super-admin continuity and representative built-in-role permissions.

## Incident correlation packet

Retain one packet containing:

- tenant, actor and target identity from trusted audit context;
- evaluator decision and required permission;
- relation-state snapshot or repair plan;
- database generation, process-applied generation and lag for each replica;
- worker running/restart state and full-clear reason;
- Redis availability and publication errors;
- the final effective-permission probes proving recovery.

Never include access tokens, session secrets, Redis credentials or private relation
payloads in logs or retained evidence.
