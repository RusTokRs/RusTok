# Two-process RBAC durable recovery evidence

## Purpose

This source packet defines one dedicated PostgreSQL integration scenario for a
missed RBAC invalidation publication across two independent operating-system
processes.

The test intentionally does not configure Redis. The mutating process publishes
to its own process-local invalidation bus after commit, while the observer process
has a separate permission cache and a separate local bus. The observer therefore
cannot receive the fast-path publication and must recover from the durable database
generation.

## Topology

`apps/server/tests/rbac_two_process_durable_recovery.rs` re-executes its own test
binary to create two independent replica probes:

- the observer process starts the canonical cache invalidation listener and durable
  generation watchdog, warms an allowed `settings:manage` snapshot for an Admin,
  and records readiness;
- the mutator process starts the same production runtime and calls
  `RbacService::replace_user_role_committed` to replace Admin with Customer;
- both processes connect to one isolated PostgreSQL database created for the test;
- neither process has Redis configuration, and their process-local buses and Moka
  permission caches cannot be shared.

The parent test only coordinates subprocess lifetime and reads bounded JSON result
files. It does not mutate relations, clear caches, advance generations, or emulate a
replica inside the parent process.

## Required observations

The scenario must retain all of the following facts:

1. the observer initially allows `settings:manage` from a warmed process cache;
2. the mutator commits the production role replacement and advances the durable
   generation exactly once;
3. after the database commit, the observer still allows from its stale cache because
   the process-local publication was intentionally missed;
4. the observer's canonical five-second durable-generation watchdog clears the
   stale process cache without a manual test hook;
5. the observer converges to deny, and its authoritative relation read also denies;
6. recovery completes within the retained seven-second integration bound.

The seven-second bound allows the production five-second poll interval plus normal
scheduler and database latency. It does not change the production interval or the
operator alert thresholds.

## Evidence boundary

The source-contract JSON remains a `source_ready_unvalidated` source-shape record.
Runtime execution is retained separately by the workflow run and artifact below.
This packet covers Redis-unavailable and intentionally missed-local-publication
recovery only. It does not prove:

- Redis publication between live replicas;
- Redis process restart or subscriber reconnection;
- CLI system-role repair propagation to live replicas;
- HTTP, GraphQL, or native transport behavior;
- broad RBAC compilation or module verification.

The full multi-replica P0 gate remains open.

## Execution

```bash
CARGO_PROFILE_TEST_DEBUG=0 cargo test --locked -p rustok-server \
  --test rbac_two_process_durable_recovery \
  separate_process_replica_recovers_missed_local_publication_from_durable_generation \
  -- --ignored --nocapture

node scripts/verify/verify-rbac-two-process-durable-recovery-source.mjs
```

`RUSTOK_MIGRATION_SMOKE_ADMIN_URL` must provide PostgreSQL database-creation
permissions. The test creates and drops a unique isolated database.

## Retained execution

PR #3570 retained a successful exact-head execution at
`b1ee738459afea328c644c10f60514f75bf96a87` in RBAC Runtime Evidence run
`31836046621`, artifact `9233262963`, with `CARGO_PROFILE_TEST_DEBUG=0`,
PostgreSQL 16, repository-selected `stable` Rust (`rustc 1.97.1`, `cargo 1.97.1`).
The source-contract verifier passed and the two-process scenario reported:

```text
test rbac_multi_replica_child ... ok
test rbac_multi_replica_child ... ok
test separate_process_replica_recovers_missed_local_publication_from_durable_generation ... ok

test result: ok. 1 passed; 0 failed
```

The parent scenario completed in 8.58 seconds while the retained observer recovery
bound remains seven seconds from the mutation/recovery checkpoint rather than from
full process and fixture startup. The successful test assertion is the authority for
that bounded recovery observation.

The PR was merged normally into `main` as
`9d7a8d4790c66bbcee3479cb880dc2008e5765b4`. The dedicated push-to-main RBAC
workflow is the preferred same-main-revision confirmation when available; the PR
artifact remains retained proof for the executable tree merged by #3570.
