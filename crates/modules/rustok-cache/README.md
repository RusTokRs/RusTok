# rustok-cache

## Purpose

`rustok-cache` centralizes cache backend lifecycle and failure policy for RusToK. It owns
Redis-backed and in-memory implementations, bounded degraded-mode fallback, anti-stampede
loading, typed/versioned values, atomic compare-and-set, invalidation recovery and cache
operability signals.

## Responsibilities

- Provide `CacheModule` metadata for the runtime registry.
- Own `CacheService`, `CacheBackendOptions`, backend selection and Redis lifecycle.
- Expose count-limited and byte-weighted in-memory backend factories.
- Bound Redis connection and command latency so cache failures cannot indefinitely occupy
  request tasks.
- Keep Redis circuit-breaker configuration centralized at the cache factory boundary.
- Provide cancellation-safe, backend-scoped request coalescing and optional deterministic
  TTL jitter/loader deadlines.
- Provide canonical tenant-aware/versioned cache keys and bounded typed value envelopes.
- Provide bounded stale-while-revalidate coordination and explicit negative caching.
- Provide atomic `CacheBackend::compare_and_set` semantics for local and Redis backends.
- Provide shared namespace generations and token-safe distributed leases.
- Provide bounded in-memory fallback during Redis degradation without treating an ordinary
  Redis miss as permission to return an unrelated stale local value.
- Keep distributed compare-and-set fail-closed when the shared primary is unavailable.
- Surface failed shared invalidation instead of silently acknowledging a potentially stale
  Redis entry.
- Provide validated namespaced invalidation publishing, versioned invalidation payloads and
  generation-gap detection.
- Expose cache health, refresh/generation statistics, invalidation counters and lightweight
  hit/miss/invalidation statistics.

## Backend factories and capacity

`CacheService::backend(prefix, ttl, max_capacity)` and
`CacheService::memory_backend(ttl, max_capacity)` preserve the historical entry-count
capacity contract.

For variable-size serialized documents, use
`CacheService::backend_weighted(prefix, ttl, max_weight_bytes)` or
`CacheService::memory_backend_weighted(ttl, max_weight_bytes)`. Weighted capacity includes
key bytes, payload bytes and per-entry metadata, preventing a small number of oversized
values from bypassing an entry-count limit.

All factories preserve the same `CacheBackend`, instrumentation and compare-and-set contract.
With Redis configured, `backend*` returns Redis primary plus the corresponding bounded
in-memory fallback; otherwise it returns the in-memory backend directly.

## Atomic compare-and-set

`CacheBackend::compare_and_set(key, expected, value, ttl)` returns an explicit
`CacheCompareAndSetOutcome`:

- `Applied` — the current bytes matched `expected` and the replacement was committed;
- `Mismatch` — the key was absent or held different bytes, so no write occurred.

Backends that cannot provide a real atomic primitive return an error. They must not emulate
CAS using an independent `GET` followed by `SET`.

In-memory writes, invalidations and CAS share bounded striped write locks. Redis uses one
binary-safe Lua command that compares the stored bytes and performs `PSETEX` or conditional
`DEL`. Circuit-breaker and operation-timeout accounting cover the complete Redis CAS command.
Instrumentation and weighted wrappers delegate the atomic primitive unchanged.

Fallback CAS is intentionally stricter than ordinary degraded writes: the shared primary is
authoritative. If Redis is unavailable, CAS returns an error and does not acknowledge a
process-local replacement. After `Applied`, the local mirror is updated; after `Mismatch`, an
old local mirror is invalidated.

## Canonical keys and typed envelopes

`CacheKeyBuilder` creates keys in the shape
`service:environment:tenant-or-global:domain:schema-version:resource:identity`. Fixed
namespace components are validated. Dynamic identities remain readable only when short and
safe; otherwise they become a SHA-256 digest. Complete keys are capped at 512 bytes, while
individual and aggregate source inputs are bounded before hashing or joining.

`CacheEnvelope<T>` uses a versioned Postcard wire format and records:

- cache envelope format version;
- domain schema version;
- generation timestamp;
- optional source revision;
- optional soft and hard expiration boundaries;
- typed payload.

Encoding first measures the serialized envelope without allocating the full output and then
writes through a bounded writer. Decoding rejects oversized input before deserialization.
Schema mismatch, unsupported format and invalid expiration ordering are explicit errors.

`CacheService::load_enveloped_or_fill` combines the envelope with coalesced loading. It
invalidates corrupted, schema-incompatible and hard-expired values before reload. A value
between soft and hard expiry is returned as `Stale` rather than silently treated as fresh.

## Fallback and consistency semantics

Writes are retained in the local fallback before the Redis write is attempted. If the Redis
write fails, a bounded marker with the same TTL records that the local value is authoritative
for degraded operation. After Redis reconnects:

- a Redis hit wins and clears the degraded marker;
- a Redis miss may use local data only while a matching degraded-write marker is alive;
- an ordinary Redis miss never returns a previously mirrored local value;
- a successful shared write clears the marker;
- invalidation always clears local state and returns an error when Redis deletion fails;
- compare-and-set requires a successful shared-primary atomic operation.

The last two rules are deliberate: callers must not report successful mutation or
invalidation while a stale shared entry may still exist on another process.

## Redis timing and TTL guarantees

Redis backend construction, GET, SET, CAS, DEL and PING are bounded by an operation timeout.
Service-level health checks, pub/sub setup/subscription, generation operations, distributed
lease operations and invalidation PUBLISH are bounded as well.

Redis TTL uses millisecond precision. Positive sub-millisecond durations are rounded up to
one millisecond; a zero TTL performs immediate invalidation instead of issuing an invalid
`PX 0`/`EX 0` command. For CAS, zero TTL performs a conditional deletion.

## Anti-stampede, avalanche and stale refresh

`CacheService::load_or_fill` coalesces only callers sharing both the same backend instance
and cache key. Identical keys belonging to different namespaces/backends do not block one
another. Gate leases are released on success, loader/storage error and future cancellation,
so cancelled tasks cannot leak in-flight keys indefinitely. Raw keys and the number of unique
in-flight gates are bounded.

`CacheService::load_or_fill_with_policy` adds:

- deterministic TTL jitter for `(namespace, key)`, bounded to ±50%;
- an optional deadline around the leader's source-of-truth loader.

The jitter is stable rather than random, so retries and tests produce the same expiry while
large namespaces avoid synchronized expiration.

`CacheRefreshCoordinator` implements bounded stale-while-revalidate:

- stale values are returned until hard expiry;
- refresh identity is `(backend, key)`;
- empty and oversized keys are rejected before permit or map allocation;
- duplicate refreshes are coalesced;
- a semaphore caps total process-local refresh concurrency;
- the exact stale envelope bytes become the CAS expectation;
- a concurrent replacement or invalidation produces `Mismatch` and wins;
- failed refreshes leave the stale value untouched;
- metrics expose started, completed, failed, deduplicated, saturated and rejected work.

For loaders whose cross-instance amplification justifies a distributed lock,
`try_acquire_distributed_lease` uses Redis `SET NX PX` with a UUID ownership token. Lease
extension and release use compare-and-PEXPIRE/delete Lua scripts, preventing one process
from modifying a lock that expired and was acquired by another owner.

## Negative caching

`NegativeCachePolicy` requires a non-zero schema version, a bounded positive TTL and an
encoded-size limit. Only explicitly classified stable domain negatives are stored through
`store_negative`; transport failures, dependency errors and timeouts cannot be implicitly
converted into cached not-found responses.

Negative entries use typed versioned envelopes. Corrupted, schema-incompatible and
hard-expired entries are invalidated and treated as misses. Deterministic TTL jitter can be
used for high-cardinality negative namespaces.

## Namespace generations and invalidation recovery

`CacheNamespaceGenerationStore` exposes a shared Redis generation counter and a bounded
local fallback snapshot. Cache keys can include `generation.key_component()`. Bumping the
counter makes all previous-generation keys unreachable without `SCAN`, wildcard deletion or
a large invalidation fan-out.

When Redis is configured, a failed bump is returned as an error: a local-only increment is
never acknowledged as cross-instance invalidation. Reads may fall back to the last locally
observed generation during Redis degradation and expose that source explicitly. Trusted
snapshots are not evicted; new namespaces fail closed after the configured process capacity.

With the `redis-cache` feature enabled,
`CacheInvalidationService::consume_subscription(channel, handler)` owns bounded Redis
pub/sub connection/subscription setup for one channel and invokes the supplied handler for
each valid invalidation message. `CacheInvalidationMessage::try_new` validates and bounds
messages before publish.

Redis pub/sub remains an at-most-once, best-effort transport: messages published while a
subscriber is disconnected are not replayed. Domain listeners must retain retry/health
telemetry and use fail-safe recovery when they detect a gap. For example, the field-definition
cache clears all entries when its event receiver reports lag.

For domains with a durable outbox/event sequence, `VersionedCacheInvalidation` carries that
monotonic generation and `CacheInvalidationGapTracker` classifies unverified-first, in-order,
duplicate, stale and gap observations. An unseeded first event requires recovery; `seed()`
accepts a persisted consumer offset. A detected gap also requires namespace recovery before
later entries are trusted.

## Interactions

- Depends on `rustok-core` for cache, atomic CAS and module contracts.
- Used by `apps/server` to build tenant, RBAC and other runtime caches.
- Does not publish its own RBAC or UI surface.
- Access to cache-backed admin operations is enforced by the host through permissions
  declared by owning domain modules.

## Entry points

- `CacheModule` / `CacheService`
- `CacheBackend` / `CacheCompareAndSetOutcome`
- `CacheService::backend` / `backend_weighted`
- `CacheService::memory_backend` / `memory_backend_weighted`
- `CacheService::load_or_fill` / `load_or_fill_with_policy`
- `CacheService::load_enveloped_or_fill`
- `CacheService::load_enveloped_stale_while_revalidate`
- `CacheKeyBuilder`
- `CacheEnvelope<T>` / `TypedCacheLoadResult<T>`
- `CacheLoadPolicy` / `CacheTtlPolicy`
- `CacheRefreshCoordinator` / `StaleWhileRevalidateResult<T>`
- `NegativeCachePolicy` / `NegativeCacheEntry<T>`
- `CacheNamespaceGenerationStore` / `CacheNamespaceGeneration`
- `CacheLeaseOptions` / `CacheLeaseOutcome` / `DistributedCacheLease`
- `CacheInvalidationMessage` / `VersionedCacheInvalidation`
- `CacheInvalidationGapTracker` / `CacheInvalidationObservation`
- `CacheInvalidationService` / `LocalCacheInvalidationSubscription`
- `CacheHealthReport` / `CacheBackendOptions`

## Verification

Source changes are not a substitute for compiled and live-service evidence. Run:

```bash
cargo fmt --all -- --check
cargo check -p rustok-core --lib
cargo check -p rustok-cache --lib
cargo check -p rustok-server --lib
cargo test -p rustok-core cache --lib
cargo test -p rustok-cache --lib
cargo test -p rustok-server --test cache_architecture_guard --test tenant_cache_architecture_guard
cargo clippy -p rustok-core --lib -- -D warnings
cargo clippy -p rustok-cache --lib -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate cache
cargo xtask module test cache
```

For the ignored Redis integration gate:

```bash
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379 \
  cargo test -p rustok-cache -- --ignored --nocapture
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:6379 \
  cargo test -p rustok-core cache -- --ignored --nocapture
```

The focused cache workflow runs formatting, compile, targeted tests, Clippy, module gates and
isolated Redis 7 integration tests. Do not claim atomic CAS verified until those jobs complete.
