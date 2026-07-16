# `rustok-cache` Documentation

`rustok-cache` is the core caching capability of the platform. It owns Redis lifecycle,
in-memory capacity policy, degraded fallback semantics, anti-stampede coordination,
invalidation transport and cache health for the host runtime.

## Purpose

- publish the canonical runtime entry type `CacheModule`;
- centralize cache backend selection and lifecycle outside `apps/server`;
- provide one failure and consistency contract for runtime modules;
- prevent cache infrastructure from becoming an unbounded latency or memory dependency.

## Scope

- `CacheService`, `CacheBackendOptions` and backend selection logic;
- count-limited and byte-weighted in-memory backends;
- Redis lifecycle, operation timeouts and configurable circuit breaker settings;
- bounded fallback semantics and shared-invalidation error propagation;
- lightweight backend instrumentation through `CacheBackend::stats()`;
- service-level Prometheus gauges for Redis configuration/health, instrumentation state,
  in-flight loaders and invalidation publish/rejection counters;
- backend-scoped, cancellation-safe `CacheService::load_or_fill` request coalescing;
- validated invalidation publishing/subscription through Redis pub/sub and local fan-out;
- absence of module-owned RBAC vocabulary or UI surface.

## Responsibility Zone

`rustok-cache` owns shared cache backend behavior, invalidation transport, and
degraded-mode semantics. Domain modules own their cache keys, payloads, and
invalidation decisions; they must not instantiate competing Redis or fallback
stacks.

## Integration

Host runtimes construct the shared `CacheService` and inject it into domain
services. Consumers use the public cache contracts and publish invalidation
through the owned service rather than coupling directly to Redis clients.

## Factory contract

Use the factory matching the payload profile:

| Factory | Capacity unit | Intended use |
| --- | --- | --- |
| `backend` | entries | small, predictable values with Redis/fallback selection |
| `memory_backend` | entries | local small-value caches |
| `backend_weighted` | bytes | variable-size serialized values with Redis/fallback selection |
| `memory_backend_weighted` | bytes | local variable-size values |

Weighted entries account for key bytes, payload bytes and per-entry metadata. Modules should
not construct Redis clients or fallback stacks themselves.

## Read/write and fallback semantics

The Redis backend is the shared source of cache truth when healthy. The in-memory layer is a
bounded degraded-mode store, not a general stale secondary source.

1. A write is placed in memory and then attempted in Redis.
2. A failed Redis write creates a bounded local degraded-write marker with the same TTL.
3. During a Redis error, reads fall back to the local value.
4. After reconnect, a Redis miss may use local data only while the matching marker is alive.
5. A normal Redis miss does not return a mirrored local value.
6. A successful Redis hit/write removes the degraded marker.
7. Invalidation clears local state first and returns the Redis deletion error when shared
   invalidation fails.

This prevents stale local shadow values from surviving ordinary Redis eviction while still
allowing values written during an outage to remain available for their bounded TTL.

## Redis latency and TTL contract

Connection manager creation and Redis GET/SET/DEL/PING operations are bounded by an
operation timeout. Cache service health, invalidation PUBLISH and pub/sub connection/
subscription setup are bounded independently at the service boundary. Timeouts are errors
and therefore contribute to circuit-breaker failure accounting.

Redis expiration uses `PX` millisecond precision. A positive duration below one millisecond
is rounded up to one millisecond. A zero duration invalidates the key immediately.

## Anti-stampede contract

`CacheService::load_or_fill` performs a first read, obtains a gate for `(backend instance,
key)`, performs a second read and invokes the loader only when the second read still misses.

The gate is scoped to the concrete backend instance so equal textual keys in different
namespaces can load concurrently. A lease owns cleanup and removes the gate after all
participants release it, including when the leading task errors or is cancelled.

This coordination is process-local. Multi-process stampede prevention for exceptionally
expensive loaders still requires a distributed lease or a durable refresh workflow.

## Invalidation and reconnect guarantees

`CacheInvalidationService::consume_subscription(channel, handler)` holds one Redis pub/sub
stream until closure/error and delivers validated `CacheInvalidationMessage` values.
Subscription setup is bounded, while the long-running receive loop intentionally remains
open until disconnect.

Redis pub/sub does not replay messages. A disconnected listener can miss invalidations even
when publishing succeeded. Domain listeners therefore own retry/backoff, health telemetry
and a fail-safe recovery action. A consumer that detects event lag or an unknown gap should
clear the affected namespace, bump a generation/version key, or rebuild from the source of
truth rather than trusting existing local entries.

In a non-Redis build, `subscribe_local()` is the baseline single-instance fan-out contract;
`subscribe_local_channel(channel)` filters one namespace. Local broadcast lag must be
handled using the same fail-safe principle.

## Observability

`CacheBackend::stats()` reports hits, misses, invalidations and current entries for
instrumented backends. Fallback statistics include the local layer instead of returning only
primary metrics.

`CacheService::prometheus_metrics()` includes:

- Redis configured and healthy gauges;
- instrumentation enabled gauge;
- active `load_or_fill` gate count;
- local invalidation publish count;
- Redis invalidation publish success/failure counts;
- invalidation validation rejection count.

A healthy local fallback does not make Redis healthy. Use `CacheService::health()` for the
shared backend signal and domain listener health for invalidation connectivity.

## Verification

Required source/compiled gates:

```bash
cargo xtask module validate cache
cargo xtask module test cache
cargo test -p rustok-cache --lib
```

Live Redis gate:

```bash
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379 \
  cargo test -p rustok-cache \
  real_redis_publish_and_subscription_share_validated_channel_contract \
  -- --ignored --nocapture
```

The live gate verifies validated publish/subscription parity. Additional operational tests
should inject delayed Redis responses, disconnect listeners and force local broadcast lag.

## Related Documentation

- [Crate README](../README.md)
- [Caching architecture](./CACHING_ARCHITECTURE.md)
- [Implementation plan](./implementation-plan.md)
- [Host cache contract inventory](./host-cache-inventory.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
