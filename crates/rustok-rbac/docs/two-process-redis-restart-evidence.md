# RBAC two-process Redis restart recovery evidence

## Status

`source_ready_unvalidated`

This source packet continues `cycle-001` at `core/rbac`. It does not advance or complete the component.

## Purpose

The durable invalidation design uses Redis Pub/Sub as a low-latency delivery path and the PostgreSQL generation as the recovery authority. The source harness in
`apps/server/tests/rbac_two_process_redis_restart_recovery.rs` exercises those responsibilities with two independent operating-system processes, one isolated PostgreSQL database, and one isolated `redis-server` process.

The test process owns the observer replica and Redis lifecycle. A child invocation of the same integration-test binary owns the mutating replica. The replicas do not share process memory, a Moka permission cache, or the local broadcast invalidation bus.

## Available Redis scenario

1. Start an isolated Redis endpoint supplied by `RUSTOK_CACHE_REDIS_SERVER_BIN`.
2. Start the observer through the production `CacheService`, RBAC invalidation listener, and permission resolver.
3. Warm an allowed `settings:manage` decision for one tenant Admin.
4. Confirm the production RBAC channel has a Redis subscriber through `PUBSUB NUMSUB`.
5. Start the mutator in a second OS process.
6. Commit Admin-to-Customer through `RbacService::replace_user_role_committed`.
7. Require one durable generation advance, successful canonical Redis publication, and observer convergence to the authoritative deny within three seconds.

The harness intentionally does not start the database watchdog. This prevents the five-second watchdog from satisfying the fast-path assertion and keeps the scenario specific to Redis delivery.

## Redis restart scenario

1. Start the same observer topology and warm the allowed decision.
2. Stop Redis after the observer has subscribed.
3. Commit Admin-to-Customer in the mutator process while Redis is unavailable.
4. Require the committed generation to advance and canonical publication to record a Redis failure rather than returning a false mutation failure.
5. Confirm the observer still serves the intentionally missed stale allow before restart.
6. Restart Redis on the same endpoint.
7. Require the supervised subscription worker to reconnect.
8. Require its existing subscriber-ready callback to read the durable PostgreSQL generation, clear the stale permission snapshot, and converge to the authoritative deny within five seconds.

The restart scenario also omits the database watchdog. Merged PR #2853 owns the separate watchdog-fallback source packet. Keeping the mechanisms isolated proves that Redis resubscription itself performs durable recovery instead of relying on a parallel test shortcut.

## Forbidden shortcuts

The harness must not:

- call `invalidate_all_user_permissions_cache` or `invalidate_user_permissions_cache`;
- update `rbac_invalidation_state` directly;
- reserve a generation outside the production committed mutation;
- call `publish_user_rbac_invalidation` or publish a hand-built RBAC invalidation;
- simulate both replicas inside one process;
- use SQLite or a test-only role mutation implementation.

The parent process may create the isolated database, start and stop Redis, spawn the mutator, inspect Redis subscriber count, and read bounded JSON results. It does not mutate authorization state, clear caches, or acknowledge generations on behalf of either replica.

## Evidence boundary

This packet adds source coverage only. It does not claim that PostgreSQL, Redis, subprocesses, Rust tests, formatting, source verifiers, workflows, or CI were executed.

It does not prove:

- retained runtime evidence from a real execution;
- CLI system-role repair propagation while replicas are live;
- Redis authentication, TLS, Sentinel, Cluster, or network-partition behavior;
- the complete RBAC compile and targeted verification gates;
- the complete multi-replica P0 gate.

`core/rbac` therefore remains `in_progress`.

## Targeted commands

```bash
cargo test -p rustok-server \
  --test rbac_two_process_redis_restart_recovery \
  -- --ignored --nocapture --test-threads=1
node scripts/verify/verify-rbac-two-process-redis-restart-source.mjs
```

Run the repository-wide targeted RBAC checks from the canonical implementation plan on the same reconciled revision before promoting this source packet to retained runtime evidence.