# RusToK caching architecture

## Goals

The cache layer must reduce source-of-truth load without becoming a source of unbounded
latency, memory growth or silent data inconsistency. The platform contract prioritizes
correctness over hit rate:

1. source-of-truth data remains authoritative;
2. cache failures degrade availability or performance, not correctness;
3. every cache has a bounded lifetime and bounded local memory footprint;
4. invalidation gaps are observable and recoverable;
5. high-cardinality or expensive misses are coalesced;
6. cache behavior is consistent across modules.

## Current topology

```text
caller
  |
  v
CacheService factory / load_or_fill
  |
  +--> instrumented CacheBackend
          |
          +--> Redis primary (shared L2)
          |      - operation timeout
          |      - circuit breaker
          |      - millisecond TTL
          |
          +--> Moka fallback (process-local L1/degraded store)
                 - per-entry TTL
                 - entry-count or byte-weight capacity
                 - degraded-write markers
```

Invalidation is distributed through validated Redis pub/sub and mirrored to a local
broadcast channel. Pub/sub is at-most-once and therefore an acceleration mechanism rather
than the sole correctness mechanism.

## Cache key contract

Every new cache namespace should follow:

```text
<service>:<environment>:<tenant-or-global>:<domain>:<schema-version>:<resource>:<identity>
```

Rules:

- include an explicit schema/version component;
- include tenant identity before user-controlled resource components;
- normalize case-sensitive identifiers at the boundary;
- never concatenate unbounded user input without validation or hashing;
- hash large query/filter payloads using a canonical serialization;
- do not share a prefix between values with different serialization contracts;
- document cardinality and expected payload size for each namespace.

A version bump is the safest emergency invalidation when the old key space can expire
naturally.

## TTL policy

TTL is a consistency budget, not only an eviction setting. Each namespace should define:

- normal positive TTL;
- negative-cache TTL;
- maximum stale/degraded lifetime;
- refresh threshold, when applicable;
- whether values are safe to serve during dependency outage.

Recommended defaults:

| Data class | Positive TTL | Negative TTL | Notes |
| --- | ---: | ---: | --- |
| authorization/security | 5-30 s | 1-5 s | invalidate synchronously; fail closed where required |
| tenant/config metadata | 1-5 min | 15-60 s | version keys and explicit invalidation |
| catalog/read models | 30 s-5 min | 5-30 s | use weighted capacity for large payloads |
| immutable/versioned assets | 1 h-24 h | none | key version is content identity |
| external API responses | dependency-specific | short | never exceed provider freshness guarantees |

To prevent cache avalanche, high-volume namespaces should add deterministic TTL jitter.
Jitter should be stable for `(namespace, key)` to keep tests reproducible and avoid changing
the TTL on retries. A suggested range is ±5-15%, capped so TTL remains positive.

## Read strategy

The default is cache-aside with request coalescing:

1. read the cache;
2. on miss, acquire the process-local `(backend, key)` gate;
3. read again after acquiring the gate;
4. load from the source of truth;
5. serialize and store with bounded TTL;
6. return the loaded value.

For expensive hot keys, add stale-while-revalidate only when the data contract explicitly
allows bounded staleness. Store soft and hard expirations in an envelope:

```text
{ schema_version, generated_at, soft_expires_at, hard_expires_at, payload }
```

Before hard expiry, one leader refreshes while other callers may receive the stale payload.
After hard expiry, callers wait for the loader or fail according to domain policy.

## Negative caching and penetration resistance

Negative caching is appropriate for stable “not found” or disabled results. Requirements:

- separate namespace from positive values;
- much shorter TTL than positive data;
- invalidate positive and negative keys together;
- do not cache transient database/network errors;
- validate identifiers before cache access;
- rate-limit high-cardinality misses where input is user-controlled.

Bloom filters may be considered only for very large immutable identity sets. They are not a
replacement for validation, rate limiting or short negative TTLs.

## Fallback semantics

The local layer is bounded degraded storage. It must not be treated as an unrestricted L1
that can override a healthy Redis miss.

- Redis hit: return Redis value and clear any degraded-write marker.
- Redis miss with a live degraded marker: return the matching local value.
- Redis miss without marker: return miss, even if an old mirrored value exists locally.
- Redis error: use the local value if present and within TTL.
- Redis write error: retain local value plus marker for the same TTL.
- shared invalidation error: clear local state and return the error to the caller.

Future L1 read-through caching should use versioned envelopes or generation tokens so a
healthy L2 miss cannot accidentally expose stale L1 data.

## Invalidation architecture

### Current fast path

- mutation commits to the source of truth;
- related cache keys are invalidated;
- an invalidation message is published to Redis pub/sub;
- local subscribers invalidate process-local entries.

### Required recovery path

Redis pub/sub does not replay. Every domain using local cache state must implement at least
one recovery mechanism:

- short bounded TTL;
- namespace generation/version check;
- full namespace clear on listener lag/reconnect gap;
- durable invalidation stream with consumer offsets;
- source-of-truth revision comparison.

For correctness-sensitive, high-write domains, the target is an outbox-backed durable
invalidation stream:

1. persist mutation and invalidation event in one database transaction;
2. dispatcher publishes to Redis Streams or the platform event transport;
3. consumers acknowledge offsets after invalidating local state;
4. reconnect resumes from the last acknowledged offset;
5. poison messages enter a dead-letter path with metrics and alerting.

Pub/sub may remain as a low-latency hint alongside the durable stream.

## Stampede protection

Process-local request coalescing is mandatory for cache-aside loaders. For multi-instance
hot keys, add a short distributed lease only where database amplification is material:

- lease key is namespaced and versioned;
- lease TTL exceeds the loader deadline but remains bounded;
- ownership token is checked on release;
- waiting callers use capped backoff and re-read the cache;
- loader failures do not populate positive or negative cache unless domain-approved;
- metrics distinguish leader, local waiter and distributed waiter paths.

Do not use an unbounded lock or a lock without an ownership token.

## Memory safety

Use entry-count capacity only for small predictable payloads. Use weighted capacity for JSON,
HTML, projections, schema documents and external responses.

The weight function must account for:

- key bytes;
- serialized payload bytes;
- envelope/metadata overhead;
- optional decompression expansion risk.

Large individual entries should have an explicit maximum serialized size. Values exceeding
that limit should bypass local caching and emit a metric rather than evicting most of the
working set.

## Serialization and compatibility

Cached values should use a versioned envelope. Deserialization failure means the entry is
invalid and must be removed. Never retry deserialization of the same bytes indefinitely.

Recommended fields:

- schema version;
- codec/content type;
- creation timestamp;
- optional source revision/ETag;
- soft/hard expiration timestamps;
- payload.

Sensitive values should not be cached unless encryption, access boundaries and retention
policy are explicit.

## Redis resilience

Every Redis operation must have:

- a deadline shorter than the request budget;
- circuit-breaker accounting;
- structured operation/namespace logging;
- failure metrics;
- bounded retry policy, normally zero retries in the synchronous request path;
- no secrets in logs.

Connection establishment and pub/sub subscription setup also require deadlines. The
long-running receive stream itself remains open until disconnect and is supervised by a
retry loop with backoff and health state.

## Observability

Minimum per-namespace metrics:

- hits and misses;
- positive and negative entries;
- loader executions and coalesced waiters;
- loader latency and failures;
- Redis operation latency/timeouts/circuit state;
- local and shared invalidation successes/failures;
- listener reconnects, lag and last successful message time;
- evictions by capacity versus expiration;
- current entry count and estimated weight;
- oversized-entry bypasses;
- stale-while-revalidate serves and refresh outcomes.

Avoid high-cardinality labels containing raw keys, tenant IDs or user IDs. Use bounded
namespace and operation labels.

## Testing strategy

### Unit tests

- TTL zero, sub-millisecond and overflow boundaries;
- weighted eviction and oversized entries;
- fallback recovery and ordinary Redis miss distinction;
- shared invalidation failure propagation;
- same-key coalescing and cross-backend isolation;
- cancellation/error cleanup;
- malformed serialization and invalidation messages.

### Integration tests

- live Redis GET/SET/DEL/PX behavior;
- operation timeout and circuit opening using a fault proxy;
- Redis restart during degraded writes;
- pub/sub disconnect/reconnect and missed-message recovery;
- multi-instance stampede load;
- invalidation after transaction commit.

### Load/chaos tests

- synchronized expiration of a hot namespace;
- large-payload pressure against weighted capacity;
- high-cardinality invalid identifiers;
- Redis latency, packet loss and connection exhaustion;
- consumer lag and event loss.

## Delivery phases

1. **Correctness baseline** — TTL precision, fallback markers, invalidation error propagation,
   cancellation-safe coalescing.
2. **Resource bounds** — Redis deadlines and byte-weighted local capacity.
3. **Contract centralization** — all backend construction through `CacheService`, common key
   and envelope helpers.
4. **Avalanche control** — deterministic TTL jitter and per-namespace loader deadlines.
5. **Recoverable invalidation** — generation tokens, then durable outbox/stream for critical
   domains.
6. **Advanced freshness** — optional stale-while-revalidate and distributed hot-key lease.
7. **Operational proof** — compiled suite, live Redis integration, load and chaos gates.

Phases 1 and 2 are represented in source. Later phases must be adopted per namespace and
verified in an environment with compilation and Redis fault injection.
