# Two-process Redis restart recovery evidence

Status: `source_ready_unvalidated`

This packet extends the active `cycle-001` `core/rbac` verification item with a
real multi-process Redis source harness. It does not advance the cycle cursor or
complete the RBAC component.

## Topology

The ignored integration test creates:

- one isolated PostgreSQL database migrated by the workspace migrator;
- one loopback `redis-server` child process on an ephemeral port;
- one long-lived observer process with its own process-local permission cache;
- two short-lived mutator processes using the same PostgreSQL database and Redis
  endpoint.

The observer and mutators are re-executions of the integration-test binary. They
do not share memory, a Moka cache, a local invalidation bus or a runtime context.
The observer itself waits for the canonical
`rbac.permissions.generation.v1` Redis subscriber through `PUBSUB NUMSUB` before
publishing its ready file, and the parent verifies that subscriber before starting
the first mutation.

## Redis fast path

The observer starts the canonical RBAC cache invalidation listener but does not start the durable-generation watchdog. It primes an allowed `settings:manage`
decision for an Admin user. A separate mutator commits `Admin -> Customer`
through `RbacService::replace_user_role_committed`.

The observer must converge to deny within three seconds. The production periodic
listener reconciliation interval is thirty seconds and no watchdog exists in the
observer, so the only available cross-process invalidation path inside this bound
is the canonical Redis PubSub delivery.

## Redis outage and restart

A second Admin user is primed in the same observer. The parent stops the isolated Redis process and a separate mutator commits `Admin -> Customer` while Redis is
unavailable. The committed database generation advances even though fast fan-out
cannot reach the observer.

Before restart, the observer records:

- an allowed cached decision;
- an authoritative denied decision.

The parent then restarts `redis-server` on the same loopback port. The existing observer process must reconnect. The production Redis subscription ready callback
reads the durable database generation and clears permission snapshots. The
observer must then converge to the authoritative deny within eight seconds.

The complete live replica sequence, measured from observer spawn through the
post-restart decision, must finish within twenty-five seconds. This is shorter
than the production thirty-second periodic listener reconciliation interval, so a
passing restart result cannot be attributed to the database poll fallback.

This proves the source shape for:

- Redis available cross-process invalidation;
- Redis unavailable post-commit behavior;
- stale observer state during the outage;
- existing-listener reconnection after a real Redis process restart;
- durable-generation recovery in the resubscribe callback.

## Forbidden shortcuts

The harness does not:

- start `start_rbac_invalidation_generation_watchdog`;
- manually clear a permission cache;
- update `rbac_invalidation_state` directly;
- publish a synthetic invalidation message;
- use a test-only role writer for either committed mutation;
- simulate two replicas with two contexts in one process.

## Evidence boundary

No Rust test, Node verifier, formatting, Cargo check, PostgreSQL runtime, Redis
runtime, subprocess runtime, workflow or CI check was executed in the connector
work unit that added this packet.

The source packet does not close:

- live CLI system-role repair propagation;
- HTTP, GraphQL, WebSocket or native transport evidence;
- same-revision compile, lint, module validate or module test gates;
- the complete `core/rbac` verification item.

The canonical cursor remains `core/rbac` and the full multi-replica P0 gate remains open until the harness is executed and retained together with the other required
gates.

## Targeted execution

```bash
cargo test -p rustok-server \
  --test rbac_two_process_redis_restart \
  separate_process_redis_fast_path_survives_restart_and_recovers_missed_publication \
  -- --ignored --nocapture
node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs
```

Required environment:

```text
RUSTOK_MIGRATION_SMOKE_ADMIN_URL=postgres://...
RUSTOK_CACHE_REDIS_SERVER_BIN=/path/to/redis-server
```
