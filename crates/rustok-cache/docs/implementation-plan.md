# Implementation plan for `rustok-cache`

## Source of truth

This file is the canonical live implementation plan for the cache capability. It contains the
current contract, source-complete work and the remaining verification or implementation backlog.
Completed execution history does not belong here.

- `[x]` means the contract exists in `main` and is protected by a regression test or source guard.
- `[ ]` means implementation or verification is still required.
- Source-complete work is not **compiled verified** until the targeted Rust gate passes on the same
  revision.
- Redis-dependent work is not **live verified** until the isolated Redis scenarios pass on that
  revision.
- Domain-specific cache identity and recovery stay in the owner module plan. This plan coordinates
  the reusable capability and host adoption only.

Last reconciled with `main`: 2026-07-17.

## Ownership boundary

- `rustok-cache` owns backend selection, Redis lifecycle/timeouts, bounded memory policy, degraded
  fallback semantics, typed envelopes, loading/refresh/lease primitives, invalidation transport,
  generation helpers and capability metrics.
- `apps/server` owns host cache instances, worker startup/supervision and readiness aggregation.
- Domain modules own cache keys, source-of-truth recovery, durable generation/cursor allocation and
  classification of stable negative results.
- Redis PubSub is an at-most-once fast path. Durable convergence requires a database generation,
  transactional outbox record or persisted stream offset owned by the affected domain.

## Current source-complete contract

### 1. Redis, TTL and backend correctness

- [x] Use Redis `PX` millisecond expiry; zero TTL deletes immediately and positive sub-millisecond
  TTL rounds up to one millisecond.
- [x] Bound connection creation, commands, health checks, PUBLISH and subscription setup with
  timeouts that participate in circuit-breaker accounting.
- [x] Reuse the `CacheService` Redis client in all shared backend factories.
- [x] Clamp canonical shared-client Redis TTL arguments to the signed Redis range.
- [x] Remove legacy URL-owned Redis/fallback implementations and public exports from `rustok-core`;
  retain only an empty compatibility feature for existing workspace commands.
- [x] Retain Redis 7 `CLIENT PAUSE` evidence proving the shared two-second operation deadline,
  immediate circuit-open rejection and successful half-open recovery after the latency clears.
- [x] Retain a self-hosted Redis fixture that drives one shared backend and connection manager through
  two stop/start cycles, requires fast open-circuit rejection on each outage and verifies health plus
  cache operations after each recovery.

### 2. Bounded degraded fallback

- [x] Keep local fallback capacity bounded by entries or byte weight, including key and metadata
  overhead.
- [x] Prefer a marked local outage write over an older Redis value only for the marker’s bounded TTL.
- [x] Never serve an ordinary mirrored local value for a healthy Redis miss.
- [x] Warm local fallback after a successful shared-primary read.
- [x] Use pending-delete tombstones so a failed Redis invalidation cannot immediately resurrect an
  older shared value.
- [x] Fail closed for fallback CAS while the shared primary is unavailable.
- [x] Keep Redis degradation visible to health/readiness while eligible reads use bounded fallback.
- [x] Retain a unit scenario where shared health is degraded while an eligible bounded local write is
  still served.
- [x] Keep one atomic core memory backend and one canonical degradation-aware shared fallback in
  `rustok-cache`, preventing semantic drift between duplicate implementations.

### 3. Stampede, refresh and leases

- [x] Coalesce `load_or_fill` by backend identity and bounded key, with a second read under the gate.
- [x] Bound unique in-flight gates and loader duration; release gates on error, panic or cancellation.
- [x] Apply deterministic TTL jitter that never extends the configured maximum TTL.
- [x] Provide bounded stale-while-revalidate coordination with cancellation-safe terminal metrics.
- [x] Provide token-owned Redis leases with compare-and-release/extend scripts.
- [x] Record bounded loader, refresh, lease and saturation metrics.

### 4. Keys, envelopes and negative results

- [x] Build canonical service/environment/scope/domain/schema/resource keys.
- [x] Reject oversized aggregate identity input before SHA-256 hashing.
- [x] Encode typed Postcard envelopes with format, schema, source and freshness metadata.
- [x] Bound encoded output before allocation/write and invalidate corrupt, incompatible or expired
  entries before reload.
- [x] Support explicit negative-cache policy with an independent TTL, schema namespace and byte
  ceiling.
- [x] Cache only owner-classified stable negative results.

### 5. Atomic publication and CAS

- [x] Expose explicit `Applied` and `Mismatch` outcomes.
- [x] Couple local comparison and mutation in one Moka `and_compute_with` operation; missing or
  expired values remain `Mismatch` and cannot be revived.
- [x] Use binary-safe Lua compare-and-write for Redis.
- [x] Delegate CAS through fallback, weighted and observability wrappers without weakening atomicity.
- [x] Publish stale refresh results through CAS and treat mismatch as a newer authoritative value.
- [x] Wire the permanent compiled gate to the full local CAS integration suite: exactly-one-winner
  contention, expired-entry non-revival, capacity eviction non-revival and 128 invalidate/CAS races.
- [x] Retain isolated Redis source evidence for binary-safe mismatch, applied replacement and
  conditional delete, plus a self-hosted outage scenario where primary CAS errors without changing
  the local mirror, CAS rejects an unsynchronized degraded write, and recovery restores shared CAS.

### 6. Durable invalidation primitives

- [x] Validate bounded versioned invalidation payloads with caller-owned monotonic generations.
- [x] Distinguish in-order, duplicate/stale, unverified-first and gap observations.
- [x] Advance applied/recovery offsets only after the owner action succeeds.
- [x] Bound tracked channels, process gates and trusted generation snapshots.
- [x] Rotate shared namespaces through Redis `INCR` without `SCAN` or wildcard deletion.
- [x] Reject shared generation read/bump failure and regression rather than acknowledging locally.

### 7. Active host cache adoption

- [x] Tenant resolution uses weighted positive/negative backends, canonical keys, typed values,
  coalescing and durable tenant generation recovery.
- [x] Tenant locale uses a byte-weighted process cache plus exact/wildcard durable tenant-generation
  recovery, supervised local/Redis/reconcile workers and critical recovery readiness. Source tests
  use two actual serving contexts to cover exact and wildcard value refresh, deterministic local
  listener lag, a completely missed PubSub publication, Redis reconnect after shared-generation
  loss, fail-closed regression, explicit epoch restoration and delivery of the next `N+1` event.
  Every event validates the durable epoch before cache mutation or tracker acknowledgement. If the
  durable epoch is already beyond an otherwise in-order exact event, the listener performs a
  namespace-wide recovery and records the durable offset instead of applying only that tenant key.
- [x] Marketplace list/detail caches are byte-weighted, hashed, response-bounded and single-flight;
  detail negatives use a short independent TTL.
- [x] Channel cache is byte-weighted with hashed request facts, bounded monotonic tenant versions,
  trigger-backed database generation reserved with every channel-table mutation, local/Redis fast
  publication, five-second database reconciliation, atomic namespace rollover, critical worker
  guardrails and fail-safe cache bypass on allocator exhaustion. Source tests cover SQLite replica
  readers, two independent server runtimes without Redis, deterministic local broadcast lag,
  combined listener-lag/readiness/Axum-value recovery without replacement publication, PostgreSQL
  commit/rollback/concurrency/replay, live Redis remote readiness, remote Axum resolved-value
  refresh, completely missed-publication polling, database generation-state loss/recovery,
  generation regression and self-hosted Redis restart/reconnect across existing replicas.
- [x] RBAC permissions use weighted typed identity, bounded striped epochs and database-backed durable
  generation recovery.
- [x] SEO redirects reconcile from transactionally persisted rows with a bounded `(created_at, id)`
  cursor, indexed paging, seed-before-clear startup and critical worker readiness. Two-replica source
  evidence covers exact tenant invalidation, multi-page catch-up, out-of-order count/cursor gap
  recovery, database outage/restart recovery and terminal isolation of one worker while the other
  remains ready.
- [x] Flex field-definition cache is byte-weighted, keeps exact local EventBus invalidation as a fast
  path and advances one transaction-local singleton generation from user/product/order/topic table
  triggers. Source tests cover all four SQLite owner triggers with reorder, soft delete, rollback,
  delete and replay; PostgreSQL statement triggers with an independent replica, rollback and two
  concurrent mutations; and two independent server caches across startup, advancement, database
  outage/recovery and generation regression. Database error or regression clears cached schemas,
  readiness remains failed, and the supervisor recovers only after a monotonic durable generation.
- [x] Rate-limit memory counters are entry-bounded; Redis identities are hashed and fail closed when
  the selected distributed backend is unavailable.
- [x] Rate-limit maintenance exits when its task owns the final limiter reference, preventing orphan
  cache retention after runtime teardown.
- [x] Redis status, channel, field-definition, tenant-locale, RBAC and SEO workers expose terminal
  lifecycle through runtime guardrails/readiness.

The detailed active-cache contract is maintained in
[`host-cache-inventory.md`](./host-cache-inventory.md).

### 8. Operations and regression guards

- [x] Maintain one permanent path-scoped `Cache hardening` workflow covering format,
  core/cache/channel/Flex-owner/server compilation, regression/architecture tests, Clippy, module
  validation, PostgreSQL 17 channel/Flex generation evidence and isolated Redis 7 jobs.
- [x] Include the complete `rustok-core` subtree in the workflow path scope so source, tests and
  manifest changes cannot bypass cache verification.
- [x] Guard the channel workflow path scope, Channel/Flex Clippy commands, PostgreSQL job, full
  non-ignored resolved-value suite, combined lag/value lib test, live Redis readiness/resolved-value
  commands, self-hosted Redis restart setup and durable recovery sources from accidental removal.
- [x] Guard tenant-locale path scope, compiled and ignored Redis commands, exact/wildcard value,
  durable-ahead namespace recovery, deterministic lag, missed publication, shared-state
  loss/restoration, critical readiness and durable-before-apply ordering from accidental removal.
- [x] Guard SEO transaction ordering, cursor index, seed-before-clear readiness, exact/multi-page/gap
  and database outage recovery, terminal-worker isolation and permanent workflow execution from
  accidental removal.
- [x] Guard Flex SQLite/PostgreSQL test paths and commands, all four owner mutation classes,
  two-replica outage/regression recovery, clear-before-ack ordering and critical `is_ready()` wiring
  from accidental removal.
- [x] Guard live Redis latency/circuit and two-restart scenarios plus the production
  timeout/open-circuit markers from accidental removal.
- [x] Guard the full local CAS command and expiry, eviction, contention and invalidation-race test
  names from accidental narrowing.
- [x] Guard binary-safe live Redis CAS, self-hosted fallback outage/recovery, unsynchronized mutation
  rejection and both isolated workflow commands from accidental removal.
- [x] Guard removal of legacy Redis/fallback definitions and root/prelude exports from accidental
  reintroduction.
- [x] Guard Redis, generation, PubSub, refresh and CAS Prometheus alert metric names in
  `tests/alert_rules_guard.rs`.
- [x] Publish operational alerts for Redis degradation, generation bump failure, PubSub failure,
  refresh saturation, CAS failure, skipped event messages and repeated worker restarts.
- [x] Publish incident response, gap recovery and safe capacity/TTL tuning guidance in
  [`operations.md`](./operations.md).
- [x] Keep host ownership and cross-replica consistency decisions documented in owner plans instead
  of duplicating domain policy here.

## Remaining work, in priority order

### P0. Compiled verification

- [ ] Run the permanent `Cache hardening` workflow on one reconciled `main` revision.
- [ ] Fix every cache-specific format, compile, test or Clippy failure found by that run.
- [ ] Record the verified revision and job results here without copying raw logs.

### P0. Live and failure-recovery evidence

- [ ] Execute the source-complete channel SQLite reader, two-server-runtime, listener-lag/value,
  resolved-value, PostgreSQL 17, live Redis, latency/circuit and repeated self-hosted Redis restart
  jobs on the same revision and record their results.
- [ ] Run ignored `rustok-cache` suites against isolated Redis 7.
- [ ] Execute and record the source-complete exact/wildcard tenant-locale, durable-ahead recovery,
  listener-lag, missed-publication, Redis state-loss/restoration and critical-readiness scenarios.
- [ ] Execute and record the source-complete Flex SQLite owner matrix, PostgreSQL
  transaction/concurrency/replay and two-replica startup/outage/regression recovery scenarios.
- [ ] Execute and record the source-complete SEO seed-before-clear, exact tenant, multi-page,
  count/cursor-gap, database outage/recovery and terminal-worker scenarios across two replicas.
- [ ] Execute and record the source-complete binary-safe Redis CAS and self-hosted fail-closed fallback
  CAS outage/recovery scenarios.
- [ ] Execute and record the source-complete degraded-health plus eligible-local-read scenario.

### P1. Load, chaos and tuning evidence

- [ ] Exercise synchronized expiry, oversized payloads, hot-key contention, refresh saturation,
  lease expiry and invalidation listener lag.
- [ ] Exercise generation snapshot capacity, generation read/bump failure and CAS contention or
  timeout behavior.
- [ ] Measure marketplace hot-slug coalescing, channel token rollover, Flex generation full-clear
  cost and SEO cursor catch-up under concurrency.
- [ ] Tune byte budgets, TTLs, jitter, negative TTLs and concurrency limits from observed workload
  distributions and latency objectives.
- [ ] Validate the initial Prometheus thresholds against production baselines and document any
  changes with rollback criteria.

### P2. Atomic local CAS execution evidence

- [x] Local CAS implementation and source guard use Moka key-level entry compute.
- [x] Permanent gate is wired to execute expiry, eviction, exactly-one-winner contention and
  concurrent invalidation/CAS stress coverage.
- [ ] Record a successful execution of the full local CAS suite on the verified revision.

## Verification commands

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib --features redis-cache
cargo check -p rustok-cache --lib
cargo check -p flex --lib
cargo check -p rustok-auth --lib
cargo check -p rustok-product --lib
cargo check -p rustok-commerce --lib
cargo check -p rustok-forum --lib
cargo check -p rustok-channel --lib
cargo check -p rustok-server --lib
cargo test -p rustok-core cache --lib --features redis-cache
cargo test -p rustok-core --test cache_atomic_backend_guard
cargo test -p rustok-cache --lib
cargo test -p rustok-cache --test alert_rules_guard
cargo test -p rustok-cache --test atomic_cas
cargo test -p rustok-cache --test invalidation_failure_metrics
cargo test -p flex cache_generation --lib
cargo test -p rustok-channel invalidation_generation --lib
cargo test -p rustok-channel sqlite_triggers_advance_generation_and_replay_preserves_it --lib
cargo test -p rustok-server channel_cache_invalidation --lib
cargo test -p rustok-server --test channel_cache_resolved_value
cargo test -p rustok-server tenant_locale_generation --lib
cargo test -p rustok-server seo_redirect_cache_reconciliation --lib
cargo test -p rustok-server field_definition_cache_generation --lib
cargo test -p rustok-server \
  --test cache_architecture_guard \
  --test cache_legacy_fallback_guard \
  --test tenant_cache_architecture_guard \
  --test marketplace_cache_architecture_guard \
  --test channel_cache_architecture_guard \
  --test locale_cache_architecture_guard \
  --test tenant_locale_generation_guard \
  --test seo_redirect_cache_reconciliation_guard \
  --test field_definition_cache_runtime_guard \
  --test field_definition_cache_generation_guard \
  --test rate_limit_cache_runtime_guard \
  --test cache_redis_monitor_architecture_guard \
  --test cache_worker_guardrail_architecture_guard
RUSTOK_CHANNEL_TEST_POSTGRES_URL=postgres://postgres:postgres@127.0.0.1:5432/rustok_channel \
  cargo test -p rustok-channel --test postgres_invalidation_generation -- --ignored --nocapture --test-threads=1
RUSTOK_FLEX_TEST_POSTGRES_URL=postgres://postgres:postgres@127.0.0.1:5432/rustok_channel \
  cargo test -p flex --test postgres_cache_generation -- --ignored --nocapture --test-threads=1
cargo clippy -p rustok-core --lib --features redis-cache -- -D warnings
cargo clippy -p rustok-cache --lib -- -D warnings
cargo clippy -p flex --lib -- -D warnings
cargo clippy -p rustok-channel --lib -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate cache
cargo xtask module test cache
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-cache -- --ignored --nocapture --test-threads=1
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-cache --test atomic_cas -- --ignored --nocapture --test-threads=1
RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server \
  cargo test -p rustok-cache --test fallback_cas_live -- --ignored --nocapture --test-threads=1
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server \
  cargo test -p rustok-server tenant_locale_generation --lib \
  -- --ignored --nocapture --test-threads=1
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-server redis_publication_drives_remote_replica_readiness_recovery \
  --lib -- --ignored --nocapture --test-threads=1
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
RUSTOK_CACHE_REDIS_SERVER_BIN=/usr/bin/redis-server \
  cargo test -p rustok-server --test channel_cache_resolved_value \
  -- --ignored --nocapture --test-threads=1
```

## Completion gates

- Source-complete work becomes **compiled verified** only after the targeted commands pass on the same
  revision.
- Redis behavior becomes **live verified** only after isolated Redis evidence passes on that revision.
- Do not claim cache hardening complete while any P0 item remains open.

## Change rules

1. Keep reusable backend wiring, invalidation primitives and fallback policy in `rustok-cache`.
2. Keep domain cache identity and recovery policy in the owning module.
3. Update the crate README, inventory, operations runbook and module plan with contract changes.
4. Update the central implementation-plan registry only for status and nearest priority.
5. Prefer a correctness-preserving miss over serving unversioned stale data.
