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

Two asymmetries are contractual, not incidental:

- `backend*` prefix physical keys with the current backend generation (`g-<n>:` / `boot-<uuid>:`),
  so `bump_cache_backend_generation` makes a whole namespace unreachable without `SCAN` or
  wildcard deletion. These prefixes are part of the key space in Redis.
- `memory_backend*` return a plain in-memory backend and are **not** generation-aware:
  `bump_cache_backend_generation` has no physical effect on them. Use `backend*` whenever
  namespace rotation must be observable.

A zero TTL or a zero capacity would silently turn a backend into a permanent miss. Factories
apply documented floors (`MIN_CACHE_BACKEND_TTL` = 1 ms, capacity ≥ 1), increment
`rustok_cache_invalid_backend_configuration_total` and log at `error` level, so a
configuration typo is visible instead of silent. `CacheService::load_or_fill` rejects a zero
TTL outright.

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
- a Redis miss may serve local data only while a matching degraded-write marker is alive;
- an ordinary Redis miss never returns a previously mirrored local value;
- a successful shared write clears the marker;
- invalidation always clears local state and returns an error when Redis deletion fails;
- compare-and-set requires a successful shared-primary atomic operation;
- when the retained-invalidation tracker saturates, every read and write fails closed with an
  explicit error instead of serving possibly-stale bytes.

Fail-closed state is left through `CacheBackend::reconcile_local_state()` (implemented by the
fallback backend and forwarded by every wrapper, aggregated by
`CacheService::reconcile_local_state()`). Reconciliation first proves shared-primary health and
then replays every retained invalidation; degraded-write markers are dropped rather than replayed
because a newer cross-instance write may exist. The service performs this reconciliation on the
same `PING` that drives `redis_status()`, so a healthy shared primary is always accompanied by a
reconciled local layer.

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

- deterministic TTL jitter for `(namespace, key)`: the configured TTL is shortened by at most
  `jitter_percent` (bounded to 50%), so the effective TTL stays inside `[ttl · (1 - p), ttl]` and
  never exceeds the configured maximum;
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
- `CacheRefreshSchedule` distinguishes `Fresh` (fresh hit), `Filled` (synchronous load) and the
  scheduled outcomes (`Spawned`, `AlreadyRunning`, `AtCapacity`, `InvalidKey`,
  `RuntimeUnavailable`), so refresh-hit rate is measurable;
- metrics expose started, completed, failed, deduplicated, saturated and rejected work;
- the refreshed envelope is validated against the clock read *after* the loader returns, and the
  stored TTL is clamped to the envelope's own hard expiry (`CacheRefreshClock::System` in
  production, `CacheRefreshClock::Fixed` for deterministic hosts/tests).

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
never acknowledged as cross-instance invalidation. If the `INCR` succeeded but the local
snapshot could not be updated, the bump reports
`CacheGenerationError::SharedBumpAppliedLocalLag { generation, detail }` — the shared counter has
already moved, so a retry would rotate the namespace twice; re-`read()` instead.

Reads may fall back to the last locally observed generation during Redis degradation and expose
that source explicitly. A shared counter *below* a trusted local observation means shared state
was lost (for example a Redis restart without persistence); the store heals it with an atomic
"advance only" script, reports the healed value as `SharedRedis` and counts the outcome in
`rustok_cache_generation_shared_catch_up_writes_total` /
`..._failures_total`. Only when the catch-up cannot be written does the namespace stay divergent
(`LocalFallback` plus a warning). Trusted snapshots are not evicted; new namespaces fail closed
after the configured process capacity.

With the `redis-cache` feature enabled,
`CacheInvalidationService::consume_subscription(channel, handler)` owns bounded Redis
pub/sub connection/subscription setup for one channel and invokes the supplied handler for
each valid invalidation message. `CacheInvalidationMessage::try_new` validates and bounds
messages before publish.

`CacheService::redis_status()` reports configuration, client initialization and transport
connectivity separately, and keeps recovery outcomes in dedicated fields:

- `connectivity_healthy` — the `PING` succeeded (pure transport signal);
- `generation_recovery_pending` — backend generations that still need a trusted shared snapshot;
- `recovery_error` — why local-state reconciliation or generation recovery failed although Redis
  answered.

`is_healthy()` (module readiness) requires reachability **and** completed recovery;
`is_connectivity_healthy()` is the diagnostic view that ignores recovery. The gauges
`rustok_cache_redis_generation_recovery_pending` and `..._failed` expose the difference, so
"Redis is down" never gets confused with "Redis is up but recovery is incomplete".

Redis pub/sub remains an at-most-once, best-effort transport: messages published while a
subscriber is disconnected are not replayed. Domain listeners must retain retry/health
telemetry and use fail-safe recovery when they detect a gap. For example, the field-definition
cache clears all entries when its event receiver reports lag.

For domains with a durable outbox/event sequence, `VersionedCacheInvalidation` carries that
monotonic generation and `BoundedCacheInvalidationGapTracker` classifies unverified-first,
in-order, duplicate, stale and gap observations. An unseeded first event requires recovery;
`seed()` accepts a persisted consumer offset. A detected gap also requires namespace recovery
before later entries are trusted. The tracker is the single owner of that invariant: it is
bounded by `DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS` and there is no second unbounded variant.

`VersionedCacheInvalidation::to_message` validates its payload through the transport
constructor, and the transport (`CacheInvalidationMessage::{try_new, validate}`) is the single
owner of the channel/key byte limits (`MAX_CACHE_INVALIDATION_CHANNEL_BYTES`,
`MAX_CACHE_INVALIDATION_KEY_BYTES`). A payload the transport would reject can therefore never be
reported as successfully built.

`CacheInvalidationService::publish` and `CacheService::publish_invalidation` return a `Result`:
an invalid message yields `CacheInvalidationError::InvalidMessage`, and a configured-but-failing
Redis publish yields `CacheInvalidationError::SharedTransport { local_subscribers, detail }`
after local fan-out succeeded. Memory-only mode is not an error (`redis_published: false`).
`publish_durable` propagates the same failure as `DurableCacheInvalidationError::Transport`, so a
domain cannot advance a generation and believe the fleet was notified. Publishing reuses one
lazy `ConnectionManager` instead of opening a TCP connection per message.

A local-only bump (no Redis configured) is deliberately process-local: it is never announced to
other instances, and a mixed deployment (some instances without Redis) must not rely on
`bump_cache_backend_generation` for cross-instance coordination. When Redis is configured and
`INCR` succeeded but the local snapshot could not be updated, the call reports the failure while
the shared counter already moved; a blind retry rotates twice, so callers should re-`read()` the
generation before retrying.

## Settings and admin surface

`CacheSettings` is the canonical settings document: it owns the effective-target resolution
(`resolved_url()`: explicit `redis_url`, then the structured host/port/database fields),
validation (scheme `redis`/`rediss`, host presence, database range, mode ↔ target agreement),
client redaction (both the password field and the password inside the URL) and merge-back of a
redacted document (`merge_with_existing` / `preserve_url_password`, which restores a password only
when the incoming document still points at the same target).

The **Redis target is deployment configuration**: the runtime resolves it once at startup
(`settings` file → `RUSTOK_REDIS_URL` → `REDIS_URL`, resolved by `resolve_redis_url`). The host
settings service therefore reports the deployment-resolved document for the `cache` category and
rejects a write that would redirect it, instead of persisting a value that nothing reads.

The cache admin server functions are real adapters, not fixtures: Redis health comes from the
runtime `CacheService` published in the host context, and settings are read/persisted through
`CacheSettingsAdminPort` (implemented by the host settings service). Both require
`settings:manage`, matching the GraphQL transport.

## Interactions

- Depends on `rustok-core` for cache, atomic CAS and module contracts.
- Used by `apps/server` to build tenant, RBAC and other runtime caches.
- Publishes a narrow admin surface (`admin/`: cache health plus the `cache` settings document)
  and no RBAC vocabulary of its own. Access is enforced by the host (`settings:manage`) on both
  the GraphQL and native transports.

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
- `BoundedCacheInvalidationGapTracker` / `CacheInvalidationObservation`
- `CacheInvalidationService` / `LocalCacheInvalidationSubscription`
- `CacheGenerationStats` / `CacheGenerationError`
- `CacheHealthReport` / `CacheBackendOptions`
- `CacheService::redis_status` / `RedisCacheStatus`
- `CacheSettings` / `CacheSettingsAdminPort` / `SharedCacheSettingsAdmin`
- `CacheService::reconcile_local_state` / `CacheBackend::reconcile_local_state`

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

## Docs

- [CRATE_API.md](CRATE_API.md)
- [Platform docs index](../../../docs/index.md)
