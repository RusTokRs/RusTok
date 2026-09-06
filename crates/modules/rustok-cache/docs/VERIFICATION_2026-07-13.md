# Cache hardening verification — 2026-07-13

This document records the verification executed after the cache hardening phases were merged
into `main`. It is evidence for the source contract; it is not a substitute for production
telemetry or a staged rollout.

## Compiled gates

The following commands completed successfully from a fresh archive of `main`:

```bash
cargo fmt --all --check
cargo check -p rustok-cache --lib
cargo check -p rustok-server --lib
cargo test -p rustok-cache --lib
cargo test -p rustok-server \
  --test cache_architecture_guard \
  --test tenant_cache_architecture_guard
cargo test -p rustok-server tenant_cache --lib
cargo test -p rustok-server channel_cache --lib
cargo test -p rustok-server rate_limit --lib
cargo test -p rustok-server tenant_locale_cache --lib
cargo test -p rustok-server permission_cache_weight --lib
cargo test -p rustok-seo redirect_cache --lib
cargo clippy -p rustok-cache --lib -- -D warnings
cargo clippy -p rustok-server --lib -- -D warnings
cargo xtask module validate cache
cargo xtask module test cache
```

The focused suites cover:

- Redis millisecond TTL conversion and operation deadlines;
- backend-scoped, cancellation-safe request coalescing;
- count and byte-weighted memory capacity;
- bounded degraded-mode fallback and invalidation failure propagation;
- typed/versioned cache envelopes and corrupted-value recovery;
- negative caching and deterministic TTL jitter;
- stale-while-revalidate duplicate-loader prevention;
- namespace generation monotonicity and invalidation gap handling;
- token-owned distributed leases;
- tenant, channel, locale, RBAC, SEO redirect and rate-limit cache contracts;
- architecture guards preventing regression to raw keys, count-only variable payload caches,
  URL-based Redis construction and unbounded Redis operations.

## Live Redis gates

An isolated local Redis server was started with persistence disabled:

```bash
redis-server \
  --bind 127.0.0.1 \
  --save '' \
  --appendonly no \
  --port <isolated-port>
```

The following ignored suites completed successfully against that instance:

```bash
RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:<isolated-port>/ \
  cargo test -p rustok-cache -- --ignored --nocapture

RUSTOK_CACHE_REAL_REDIS_URL=redis://127.0.0.1:<isolated-port>/ \
  cargo test -p rustok-core cache -- --ignored --nocapture
```

Live coverage includes:

- validated Redis pub/sub publish and subscription;
- shared monotonic namespace generations across independent `CacheService` instances;
- token-safe distributed lease contention, release and reacquisition;
- shared-client weighted backend sub-second expiry and invalidation;
- Redis-backed core cache behavior covered by the ignored cache tests.

## Source contract verified

At this point the cache source contract provides:

- one `CacheService`-owned Redis client path for count and weighted factories;
- bounded connection/command latency and circuit-breaker accounting;
- exact positive TTL handling down to one millisecond and zero-TTL invalidation;
- Redis `GET` + `PTTL` reads so L1 warm-up never extends source TTL;
- explicit degraded primary health instead of masking Redis failure behind L1;
- backend-and-key scoped coalescing with cancellation cleanup;
- byte-weighted capacity for variable-size payloads;
- canonical bounded keys, schema-versioned envelopes and bounded decoding;
- deterministic jitter, negative-cache policy and stale-while-revalidate primitives;
- recoverable namespace generations and two-phase invalidation gap acknowledgement;
- token-safe Redis leases using ownership-checked Lua release/extend;
- hardened tenant, channel, locale, RBAC, SEO redirect and rate-limit cache paths.

## Residual operational risks

These risks require deployment and platform work rather than another isolated cache helper:

1. **Mutation/outbox coupling.** A domain mutation and its generation bump are not one database
   transaction. Tenant invalidation retries a failed generation bump in-process, but a process
   crash between the mutation commit and retry still requires replay from the domain outbox.
2. **Adoption by every domain.** The generic generation/envelope/negative/SWR APIs exist, but
   domains with local-only caches must adopt them when cross-instance freshness is required.
3. **Network fault injection in CI.** Unit tests bound stalled futures and live tests use real
   Redis, but a CI Toxiproxy/packet-loss/restart matrix should continuously verify latency and
   reconnect behavior.
4. **Production sizing.** Byte budgets and TTLs need validation against real payload
   distributions, hit ratios, eviction rates and Redis memory pressure.
5. **Rollout safety.** Schema-version/key-generation changes should be deployed gradually with
   dashboards and rollback thresholds for miss rate, loader latency and source-of-truth load.

## Recommended production alerts

Alert on sustained changes in:

- Redis health and circuit-breaker state;
- Redis operation timeouts;
- invalidation publish failures and listener degradation;
- generation bump/read failures and pending tenant generation rotation;
- cache loader timeouts and in-flight coalesced loads;
- hit/miss ratio, eviction rate and weighted capacity pressure;
- rate-limit backend unavailability;
- source-of-truth query latency after cache schema/key rollouts.
