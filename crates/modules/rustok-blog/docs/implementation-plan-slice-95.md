# Blog implementation plan — slice 95

Status: `recovery_postgres_evidence_source_ready_maintainer_execution_pending`.

## Purpose

Retain executable PostgreSQL evidence for the request-bound source recovery contract merged in slice 94.

This slice adds no production recovery behavior. It mounts one `cfg(test, feature = "mod-blog")` evidence module inside the existing Comments schedule audit operator module so the tests exercise the same private composition boundary and public storage APIs without publishing another runtime capability.

## Database isolation

The evidence harness reads `RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL`, falling back to a PostgreSQL `DATABASE_URL`.

Each scenario:

1. creates a unique PostgreSQL schema;
2. opens single-connection SeaORM handles with that schema as `search_path`;
3. applies the real ordered Blog audit migrations `000007` through `000011` when storage is required;
4. seeds only bounded source rows admitted by those migrations;
5. drops the schema after the scenario completes.

The authorization-order scenario intentionally uses an empty schema. Denied calls must therefore return request-bound authorization errors instead of a storage error; an authorized call must reach storage and return the bounded `Unavailable` recovery error.

## Retained scenarios

### Authorization before validation and storage

The harness proves:

- absent request authority returns `MissingRequestAuthority` before empty-schema access;
- `modules:read` returns `Forbidden` before empty-schema access;
- a control-plane tenant mismatch returns `TenantMismatch` before permission lookup;
- an authorized inspection reaches the empty schema and returns bounded `Unavailable`;
- an invalid requeue request is denied before DTO validation when authority is absent;
- the same invalid request reaches bounded `InvalidRequest` only after `modules:manage` authorization.

### Exact inspection and atomic audited requeue

For one terminal `attempt_budget_exhausted` source row, the harness verifies that inspection returns only the exact request ID, attempt count, recovery epoch, closed failure code, and closed dead-letter reason.

An exact requeue must atomically:

- reset source attempt count to zero;
- increment the recovery epoch from zero to one;
- clear claim, deferred retry, failure, and dead-letter state;
- append one audit fact with the exact control-plane tenant, actor, reason, prior attempt count, and new epoch.

PostgreSQL update and delete attempts against that audit fact must be rejected by the migration-owned append-only triggers. The recovered source row must no longer be visible through dead-letter inspection.

### Closed stale and non-terminal outcomes

The harness retains separate evidence that:

- a stale attempt count returns `StaleInspection`;
- a stale recovery epoch returns `StaleInspection`;
- a pending source row returns `NotDeadLetter`;
- none of those outcomes mutates source state or appends a recovery audit.

### Concurrent single epoch and worker admission

Two request-authorized operator calls race on separate PostgreSQL connections with the same exact inspection facts.

The row-level recovery lock and repeated terminal fence must produce exactly one `Requeued` result and one closed loser (`NotDeadLetter` or `StaleInspection`). Exactly one recovery audit and recovery epoch one must remain.

The harness then invokes the same `claim_next_retry_ready()` method used by the slice-93 worker. The recovered row must be claimed as attempt one while preserving recovery epoch one, proving that recovery resets the source retry budget without creating a replacement source row or a second worker lane.

## Preserved boundaries

This slice does not change:

- the operator, recovery store, handoff owner, source retry policy, or worker cycle;
- server startup, lifecycle reservation, shutdown, or task ownership;
- the canonical writer or atomic canonical publication transaction;
- registered event contracts or digests;
- `rustok-outbox` relay, retry, DLQ, retention, or `sys_events` schema;
- Comments listener, schedule replacement, manifests, or dependency topology;
- HTTP, GraphQL, CLI, MCP, or admin transport.

The fake canonical writer exists only to satisfy construction of the handoff owner in a test that calls `claim_next_retry_ready()` and never invokes publication.

## Maintainer execution

```bash
export RUSTOK_BLOG_COMMENTS_AUDIT_TEST_DATABASE_URL=postgres://...

cargo test -p rustok-server \
  --no-default-features \
  --features mod-blog \
  comments_provider_runtime::keyring_schedule_audit_operator::retained_postgres_evidence \
  -- --ignored --nocapture --test-threads=1

node scripts/verify/verify-blog-comments-audit-recovery-postgres-evidence.mjs
```

Cargo checks, Rust tests, JavaScript verifier, formatting, Clippy, migration application, PostgreSQL scenarios, authorization scenarios, concurrency scenarios, workflows, runtime, and production validation were not executed by the implementation agent.

## Next cursor

After successful maintainer execution, retain restart and ambiguous-commit evidence independently from canonical relay evidence. Then define source-row and recovery-audit retention without weakening append-only audit ownership or introducing a second relay.
