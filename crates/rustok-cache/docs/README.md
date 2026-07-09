# `rustok-cache` Documentation

`rustok-cache` is the core caching module of the platform. It holds Redis lifecycle,
fallback/in-memory cache semantics and cache health contract for the host runtime.

## Purpose

- publish the canonical runtime entry type `CacheModule`;
- centralize cache backend selection and lifecycle outside `apps/server`;
- provide the platform with a unified cache service contract for runtime modules.

## Scope

- `CacheService`, `CacheBackendOptions` and backend selection logic;
- Redis lifecycle, configurable circuit breaker settings, fallback semantics and cache health reporting;
- lightweight backend instrumentation via `CacheBackend::stats()` for hits/misses/invalidations/entries;
- service-level Prometheus gauges via `CacheService::prometheus_metrics()` for Redis configuration/health, metrics toggle, in-flight `load_or_fill` loaders and invalidation publish/rejection counters;
- generic anti-stampede helper `CacheService::load_or_fill`, which coalesces concurrent misses by cache key and returns the result source (`Hit`, `Filled`, `Coalesced`);
- generic invalidation publisher/subscriber `CacheService::publish_invalidation` / `CacheInvalidationService`, which validates non-empty channel/key, counts local publish / Redis success/failure / rejected counters, publishes namespaced invalidation messages to Redis pub/sub when backend is enabled, always fan-outs the message to local subscribers in the current process, supports channel-scoped local subscriptions via `subscribe_local_channel()` and gives host/runtime listeners a unified `consume_subscription` adapter for Redis pub/sub without direct Redis wiring;
- tenant-aware cache namespace and invalidation contract;
- absence of its own RBAC vocabulary and UI surface.

## Integration

- depends on `rustok-core`, `moka`, `tokio`, optional `redis` and shared infra;
- used by `apps/server` as the platform cache capability for tenant/RBAC/runtime caches;
- remains `ui_classification = "capability_only"` and does not publish module-owned UI;
- access to admin-facing cache operations is authorized by the host layer or owning module.

## Verification

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests for cache backend selection, stats instrumentation, load coalescing, invalidation message validation, invalidation publishing/local fan-out, channel-scoped local subscriptions, circuit breaker options and health semantics when changing wiring

## Related documents

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Manifest Layer Contract](../../../docs/modules/manifest.md)

## Listener/reconnect guarantees

`CacheInvalidationService::consume_subscription(channel, handler)` holds a single Redis pub/sub stream until closure or error and delivers each message to the domain handler as a `CacheInvalidationMessage`. `CacheInvalidationMessage::try_new()` provides a validated constructor for call sites that want to fail-fast before publish; `publish()` additionally drops empty channel/key without local/Redis fan-out and increments the rejected counter. The Redis subscription adapter also rejects an empty channel before connecting and ignores invalid pub/sub payloads before calling the domain handler. Retry/backoff remain with the host/runtime listener so that each domain can publish its own health status and reconnect telemetry; `apps/server` tenant listener uses this adapter inside the existing retry-loop. `CacheInvalidationService::stats()` and `CacheService::prometheus_metrics()` publish counters for local fan-out, Redis publish success/failure and rejected messages. In a non-Redis build the subscription adapter is unavailable, full local fan-out via `subscribe_local()` remains the baseline contract for single-instance runtime and tests, and `subscribe_local_channel(channel)` provides a namespace-filtered receiver for multi-listener scenarios without Redis.

## Optional real-Redis gate

When compilation and external Redis are enabled, the current ignored gate for multi-instance/pub-sub parity is run as:

```bash
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379 cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture
```

The gate verifies that `publish_invalidation` and `consume_subscription_with_ready` work through a single validated channel contract and deliver the key payload via Redis pub/sub.
