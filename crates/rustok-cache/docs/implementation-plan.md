# Implementation plan for `rustok-cache`

## Current state

`rustok-cache` is the capability-only core owner of backend selection, in-memory fallback,
Redis integration, invalidation and anti-stampede loading. The host consumes the cache
contract; it must not distribute backend-specific wiring or invalidation policy across
unrelated modules.

The source contract now includes:

- Redis millisecond TTL with zero-TTL invalidation and positive sub-millisecond rounding;
- bounded Redis connection and command latency in both core backends and service-level
  health/invalidation paths;
- circuit-breaker accounting for command timeouts;
- bounded degraded-write markers that distinguish outage writes from ordinary Redis misses;
- shared invalidation error propagation instead of silent success;
- fallback statistics that include local entries;
- backend-scoped, cancellation-safe `load_or_fill` gates;
- count-limited and byte-weighted Moka backends exposed through `CacheService`;
- fail-safe full invalidation of field-definition cache state after event-consumer lag;
- synchronized crate and architecture documentation.

Redis pub/sub remains a best-effort at-most-once fast path. Durable replay/generation
recovery is not yet a generic cache capability.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This capability has no module-owned UI or published FBA provider contract.

## Completed source phases

1. **TTL correctness**
   - Redis uses `PX` millisecond expiration.
   - zero TTL invalidates immediately;
   - positive sub-millisecond TTL rounds up to 1 ms.

2. **Fallback consistency**
   - writes made during Redis failure are tracked by a same-TTL marker;
   - ordinary Redis misses do not return stale mirrored local data;
   - shared invalidation failures are returned to callers;
   - fallback statistics include local state.

3. **Invalidation gap safety**
   - field-definition event lag clears the complete local schema cache;
   - pub/sub limitations and required recovery behavior are documented.

4. **Anti-stampede lifecycle**
   - gate identity includes backend instance and key;
   - different backends do not block on equal keys;
   - RAII cleanup covers success, errors and task cancellation.

5. **Resource bounds**
   - Redis backend and cache-service operations have deadlines;
   - local caches support byte-weighted capacity;
   - weighted factories remain centralized in `CacheService`.

## Open results

1. **Run compiled cache contract coverage.** Execute the targeted unit suite for backend
   selection, count/weighted capacity, fallback, TTL boundaries, Redis timeout helpers,
   `load_or_fill`, invalidation validation, metrics and health semantics.

   **Depends on:** an environment where compilation is available.

   **Done when:**

   ```bash
   cargo xtask module validate cache
   cargo xtask module test cache
   cargo test -p rustok-cache --lib
   ```

   pass without skipped relevant coverage.

2. **Collect real Redis evidence.** Run the ignored publisher/subscription scenario and
   backend TTL/timeout tests against an isolated Redis service.

   **Depends on:** `RUSTOK_CACHE_REAL_REDIS_URL`, isolated Redis and preferably a fault proxy.

   **Done when:** validated publish/subscription, PX expiry, reconnect and delayed-operation
   scenarios pass with observable metrics.

3. **Eliminate duplicate Redis backend construction.** Build all Redis backends from the
   `CacheService`-owned client rather than reopening a client from the URL for each namespace.

   **Done when:** count and weighted factories share the central client constructor and no
   backend factory needs the raw URL after service initialization.

4. **Add common key/envelope helpers.** Provide canonical namespace/version/key hashing and a
   versioned serialized envelope with optional source revision and soft/hard expiry fields.

   **Done when:** new module caches can adopt the helper without hand-built key formats.

5. **Add deterministic TTL jitter.** Apply opt-in stable jitter per `(namespace, key)` to
   spread expiration of high-volume namespaces without nondeterministic tests.

   **Done when:** jitter bounds, positivity and deterministic output have unit coverage and
   can be enabled through backend/load options.

6. **Add recoverable invalidation.** Introduce namespace generation tokens as an immediate
   recovery primitive, then an outbox-backed durable invalidation stream for
   correctness-sensitive domains.

   **Done when:** a disconnected instance can detect/recover from missed invalidations rather
   than relying only on TTL.

7. **Add advanced hot-key freshness controls.** Provide opt-in stale-while-revalidate and a
   token-safe distributed lease for loaders whose cross-instance amplification justifies it.

   **Done when:** soft/hard expiration, leader failure and lease ownership are tested.

8. **Operational proof.** Add load and chaos gates for synchronized expiry, oversized
   payloads, Redis latency/restart and invalidation listener lag.

## Verification

```bash
cargo xtask module validate cache
cargo xtask module test cache
cargo test -p rustok-cache --lib
RUSTOK_CACHE_REAL_REDIS_URL=redis://... \
  cargo test -p rustok-cache \
  real_redis_publish_and_subscription_share_validated_channel_contract \
  -- --ignored --nocapture
```

## Change rules

1. Keep backend wiring, invalidation and fallback policy in this module.
2. Update the crate README, local docs and `rustok-module.toml` with a cache contract change.
3. Update `docs/modules/registry.md` if module ownership or capability status changes.
4. Do not claim cache hardening complete until compiled and live Redis evidence is recorded.
5. Prefer correctness-preserving misses over serving unversioned stale values.
