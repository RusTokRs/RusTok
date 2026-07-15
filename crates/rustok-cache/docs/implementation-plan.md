# Implementation plan for `rustok-cache`

## Source of truth

This file is the canonical live implementation plan for the cache capability. It owns the
current cache contract, completed source phases, remaining priorities and targeted verification.

- `[x]` means the capability is present in `main` and protected by source-level tests,
  regression tests or architecture guards.
- `[ ]` means implementation or verification is still required.
- Source-complete work is not considered compiled or operationally verified until the
  corresponding Rust and live-service gates pass on the same revision.
- `docs/modules/implementation-plans-registry.md` contains only the current status and nearest
  priority; it must not duplicate this backlog.
- Domain-specific invalidation and worker ownership remain in the owner module plan. In
  particular, RBAC policy recovery belongs to the `rustok-rbac` plan and event-forwarder
  lifecycle belongs to the events/runtime owner; neither is duplicated here.

Last reconciled with `main`: 2026-07-15.

## Current state

`rustok-cache` is the capability-only core owner of backend selection, Redis lifecycle,
in-memory capacity policy, degraded fallback semantics, typed cache values, anti-stampede
loading, refresh/lease primitives, invalidation transport and cache health.

The ownership boundary is:

- `rustok-cache` owns reusable cache backends, factories, consistency policy, limits,
  invalidation primitives, metrics and live Redis contracts;
- `apps/server` owns host adapters, cache instances for host-owned read paths, worker startup,
  lifecycle supervision and readiness aggregation;
- domain modules own cache identity, source-of-truth recovery, durable generation allocation
  and classification of cacheable negative results;
- Redis PubSub remains a best-effort at-most-once fast path. Durable replay and recovery must
  come from a domain-owned database generation, transactional outbox or persisted stream offset.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This capability has no module-owned UI or published FBA provider contract.

## Consolidated implementation phases

### Phase 1. TTL, latency and Redis command correctness — source complete

- [x] Use Redis `PX` millisecond expiration.
- [x] Treat zero TTL as immediate invalidation.
- [x] Round positive sub-millisecond TTL up to 1 ms.
- [x] Bound Redis connection creation, commands, health checks, invalidation publishing and
  subscription setup.
- [x] Count command timeouts as circuit-breaker failures.
- [x] Reuse the Redis client owned by `CacheService` in default, count and weighted factories.
- [x] Reject restoration of the legacy URL-based namespace backend factory through architecture
  guards.

### Phase 2. Degraded fallback consistency and health — source complete

- [x] Track writes accepted while Redis is unavailable with bounded same-TTL degraded markers.
- [x] Prefer a marked local outage write over an older primary value until the marker expires.
- [x] Do not serve mirrored local values for ordinary healthy Redis misses.
- [x] Warm the bounded local layer after a successful shared-primary read.
- [x] Clear local state first and return shared invalidation/delete failures to callers.
- [x] Include local fallback state in backend statistics.
- [x] Keep Redis degradation visible to health/readiness while request paths use bounded local
  fallback.

### Phase 3. Anti-stampede, expiry avalanche and hot-key controls — source complete

- [x] Scope `load_or_fill` gates by backend instance and key.
- [x] Make gate cleanup cancellation-safe through RAII ownership.
- [x] Bound raw keys, unique in-flight gates and leader loader duration.
- [x] Provide deterministic per-key TTL jitter through `CacheLoadPolicy`.
- [x] Provide bounded stale-while-revalidate coordination with per-key refresh deduplication.
- [x] Provide token-owned Redis leases with compare-and-release/extend scripts.
- [x] Record loader, refresh and lease rejection/failure/saturation metrics.

### Phase 4. Key, envelope and negative-cache compatibility — source complete

- [x] Build canonical service/environment/scope/domain/schema/resource cache keys.
- [x] Hash unsafe identities with SHA-256 while rejecting oversized aggregate inputs and
  excessive dynamic components before hashing.
- [x] Encode typed Postcard envelopes with format/schema/source/freshness metadata.
- [x] Measure serialization before allocation and write through a bounded output writer.
- [x] Invalidate corrupted, incompatible and hard-expired typed entries before reload.
- [x] Support explicit negative-cache policy with independent TTL, schema namespace and size
  limit.
- [x] Cache only explicitly classified stable negative results.

### Phase 5. Invalidation and generation recovery primitives — source complete

- [x] Validate versioned invalidation payloads carrying caller-owned monotonic generations.
- [x] Detect unverified first events, stale/duplicate events and generation gaps.
- [x] Advance durable offsets only after the caller acknowledges successful recovery.
- [x] Rotate shared namespaces through Redis `INCR` without `SCAN` or wildcard deletion.
- [x] Reject failed shared bumps and shared generation regression instead of acknowledging them
  locally.
- [x] Bound trusted local generation snapshots without evicting already trusted namespaces.
- [x] Clear the complete field-definition cache after local consumer lag or unverified gaps.

### Phase 6. Atomic refresh publication — source complete

- [x] Expose explicit `Applied` and `Mismatch` compare-and-set outcomes on `CacheBackend`.
- [x] Fail closed for backends that cannot provide atomic CAS.
- [x] Serialize in-memory writes and CAS through bounded striped locks.
- [x] Use binary-safe Lua compare-and-write for legacy and service-owned Redis backends.
- [x] Prevent fallback CAS from acknowledging a process-local write while the shared primary is
  unavailable.
- [x] Delegate CAS through instrumentation and weighted wrappers.
- [x] Publish stale refreshes through CAS and treat mismatch as an authoritative newer value.

### Phase 7. Resource bounds and centralized factories — source complete

- [x] Provide count-limited and byte-weighted Moka backends through `CacheService`.
- [x] Account for key bytes, payload bytes and per-entry metadata in weighted entries.
- [x] Keep active tenant, channel, locale, RBAC, SEO and field-definition caches bounded by the
  appropriate capacity unit.
- [x] Keep invalidation, loader, refresh and generation tracking structures explicitly bounded.
- [x] Centralize backend construction so host modules do not create Redis clients or fallback
  stacks directly.

### Phase 8. Host cache adoption completed slices — source complete

- [x] Use byte-weighted, bounded, hashed and single-flight marketplace catalog-list caching.
- [x] Add a separate byte-weighted module-detail cache with hashed keys, single-flight loading and
  a short independently configurable negative TTL.
- [x] Bound channel tenant-generation state and clear/rotate fail-safe before any token reuse.
- [x] Disable the channel cache fail-safe if the monotonic generation allocator is exhausted.
- [x] Register channel and locale cache instances atomically on concurrent first use.
- [x] Own the field-definition cache and invalidation consumer in one restartable runtime bundle.
- [x] Supervise the Redis health/status monitor with serialized restartable startup and
  abort-on-drop ownership.
- [x] Surface terminal RBAC/cache worker state through runtime guardrails and readiness.
- [x] Preserve local invalidation delivery while recording exactly one Redis publish failure when
  Redis publication is unavailable.

### Phase 9. Source guardrails and regression coverage — source complete, execution pending

- [x] Add unit and regression coverage for TTL, degraded writes, invalidation errors, generation
  gaps, key/envelope limits, negative caching, CAS, refresh, leases and live Redis behavior.
- [x] Add server architecture guards for canonical cache ownership, tenant cache policy,
  marketplace caching, channel generations, locale registration, field-definition runtime,
  Redis monitor supervision and worker-readiness escalation.
- [x] Add path-scoped workflows for cache hardening and the new host architecture guards.
- [x] Run deterministic rustfmt over the cache-hardening file set and commit the resulting style
  normalization.
- [ ] Execute the reconciled Rust compile/test/Clippy gates and resolve cache-specific failures.
- [ ] Execute the ignored real-Redis suites and record successful publication, subscription, CAS,
  reconnect and failure-accounting evidence.

## Remaining work, in priority order

### P0. Replace temporary diagnostics with durable verification evidence

- [ ] Remove the temporary cache diagnostic/formatter workflows, trigger placeholder files and
  diagnostic issue after their useful evidence is captured.
- [ ] Keep one permanent path-scoped `Cache hardening` workflow covering format, core/cache/server
  compilation, targeted tests, Clippy, module validation and module tests.
- [ ] Run the permanent gate on one reconciled `main` revision after unrelated workspace compile
  blockers are resolved.
- [ ] Fix every cache-specific compile, lint or test failure before marking source-complete phases
  compiled verified.
- [ ] Record the exact verified revision and job results in this plan without copying raw logs.

Current external blocker: server compilation stops before cache-host code in
`crates/rustok-inventory/src/ports.rs`, where `col_expr` receives
`Expr::current_timestamp()` instead of the required `SimpleExpr`. This is not a cache contract
failure, but the server compile gate must be rerun after the workspace baseline is restored.

### P0. Live Redis and failure-recovery evidence

- [ ] Run the ignored `rustok-cache` and `rustok-core` suites against an isolated Redis 7 service.
- [ ] Prove validated channel-scoped publish/subscription parity and local delivery during Redis
  publication failure.
- [ ] Prove binary-safe CAS applied/mismatch behavior and fail-closed fallback behavior.
- [ ] Exercise Redis latency, disconnect, restart, listener reconnect and circuit-breaker recovery.
- [ ] Confirm that readiness continues to expose shared-primary degradation while bounded local
  fallback serves eligible requests.

### P1. Complete correctness-sensitive host adoption

- [ ] Inventory every active host/domain cache and classify its payload size, source of truth,
  invalidation scope, negative-result stability and cross-replica consistency requirement.
- [ ] Migrate remaining variable-size caches to byte-weighted factories and remaining dynamic
  identities to canonical bounded keys.
- [ ] Use typed envelopes and explicit load/negative policy where incompatible payloads or stale
  schema versions could change behavior.
- [ ] Add shared/durable generations only where a process-local invalidation miss can serve stale
  correctness-sensitive data on another replica.
- [ ] Keep each domain-specific recovery action in its owner module plan; do not add a second
  generic invalidation policy in the host.

### P1. Durable recoverable invalidation adoption

- [x] Provide reusable versioned invalidation, generation-gap and acknowledgement primitives.
- [x] Provide field-definition full-clear recovery after consumer lag.
- [x] Integrate a database-backed durable generation for RBAC in the RBAC owner path.
- [ ] Connect remaining eligible domain consumers to transactional outbox generations or persisted
  stream offsets.
- [ ] Seed each such consumer from persisted state before accepting fast-path invalidations.
- [ ] Execute an owner-defined rebuild, namespace rotation or full clear on `UnverifiedFirst` and
  `Gap`, then acknowledge only after recovery succeeds.

### P1. Operational proof and capacity tuning

- [ ] Add load/chaos gates for synchronized expiry, oversized payloads, hot-key contention,
  refresh saturation, lease expiry and invalidation listener lag.
- [ ] Exercise generation snapshot capacity, generation read/bump failure and CAS contention or
  timeout behavior.
- [ ] Measure marketplace hot-slug coalescing and channel generation rollover under concurrency.
- [ ] Tune byte budgets, TTLs, jitter, negative TTLs and concurrency limits from observed
  production payload distributions and latency objectives.
- [ ] Publish operator guidance and alert thresholds for Redis degradation, repeated worker
  restarts, invalidation gaps, refresh saturation and generation recovery.

### P2. Local CAS expiry/eviction proof

- [ ] Stress Moka expiration and eviction while compare-and-set is in progress.
- [ ] Prove that an expired or evicted value cannot be revived between comparison and replacement.
- [ ] If the invariant cannot be proven, move local CAS to a backend primitive that couples value
  comparison and entry mutation atomically.

## Verification commands

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo check -p rustok-cache --lib
cargo check -p rustok-server --lib
cargo test -p rustok-core cache --lib
cargo test -p rustok-cache --lib
cargo test -p rustok-server \
  --test cache_architecture_guard \
  --test tenant_cache_architecture_guard \
  --test marketplace_cache_architecture_guard \
  --test channel_cache_architecture_guard \
  --test locale_cache_architecture_guard \
  --test field_definition_cache_runtime_guard \
  --test cache_redis_monitor_architecture_guard \
  --test cache_worker_guardrail_architecture_guard
cargo clippy -p rustok-core --lib -- -D warnings
cargo clippy -p rustok-cache --lib -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate cache
cargo xtask module test cache
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-cache -- --ignored --nocapture
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379/ \
  cargo test -p rustok-core cache -- --ignored --nocapture
```

## Completion gates

- Source-complete phases become **compiled verified** only after the targeted commands pass on the
  same revision.
- Redis-dependent behavior becomes **live verified** only after the isolated Redis scenarios pass
  on that revision.
- Host adoption is complete only when every correctness-sensitive active cache has an explicit
  capacity, key, value, invalidation and degraded-mode contract.
- Do not claim cache hardening complete while any P0 item remains open.

## Change rules

1. Keep reusable backend wiring, invalidation primitives and fallback policy in this module.
2. Keep domain cache identity and recovery policy in the owning module.
3. Update the crate README, local docs and `rustok-module.toml` when the cache contract changes.
4. Update `docs/modules/implementation-plans-registry.md` only for status and nearest priority.
5. Prefer correctness-preserving misses over serving unversioned stale values.
