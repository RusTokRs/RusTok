# `rustok-cache` — Implementation Plan

Status: core cache baseline locked; module brought to mandatory
manifest/doc contract.

## Execution checkpoint

- Current phase: runtime_hardening
- Last checkpoint: Invalidation contract strengthened with validation guardrails, channel-scoped local subscriptions, service-level counters and source-level real-Redis integration scenarios; Redis subscription adapter now rejects empty channels before connection and drops invalid payloads without calling the handler. Tenant anti-stampede path already switched to `CacheService::load_or_fill`, and cache capability exports service-level Prometheus gauges for Redis health/configuration, metrics toggle, in-flight loaders and invalidation counters.
- Next step: Add compile/test evidence when the compilation restriction is lifted and run the ignored real-Redis scenario with `RUSTOK_CACHE_REAL_REDIS_URL` over the channel-scoped subscription contract.
- Open blockers: Compile/test evidence deferred by explicit iteration constraint: no compilations.
- Hand-off notes for next agent: Run `cargo test -p rustok-cache --lib` when compilations are allowed; then run `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture` for real-Redis pub/sub evidence.
- Last updated at (UTC): 2026-06-24T00:00:00Z

## Scope of work

- maintain `rustok-cache` as a capability-only core module without its own UI;
- synchronize cache backend contract, local docs and manifest metadata;
- expand cache semantics without spreading backend wiring across the host layer.

## Current state

- `CacheModule` and `CacheService` already extracted from `rustok-core`;
- module publishes a unified cache backend contract for the runtime;
- root `README.md`, local docs and `rustok-module.toml` are part of the scoped audit path;
- Redis support remains an optional feature, and the in-memory/fallback path is part of the base contract.

## Stages

### 1. Contract stability

- [x] return `rustok-module.toml` to the module standard path;
- [x] align local docs and root README under a unified contract;
- [x] maintain sync between backend contract and host integration tests via instrumented `CacheBackend::stats()` contract and documented verification debt.

### 2. Runtime hardening

- [x] complete anti-stampede coalescing;
- [x] complete circuit breaker for Redis backend at the cache factory options level;
- [x] add generic Redis pub/sub invalidation publisher and local fan-out contract;
- [x] complete generic subscription/listener adapter for Redis pub/sub invalidation between instances;
- [x] add validation guardrails for invalidation messages and channel-scoped local subscription parity;

### 3. Operability

- [x] bring Prometheus metrics export to a production-ready service-level layer, including invalidation publish/rejection counters;
- [x] add baseline hit/miss/invalidation/entry stats and health diagnostics to the cache factory contract;
- [x] add source-level ignored real-Redis pub/sub integration scenario for publish/subscription parity;
- [x] document publisher/local fan-out guarantees for the generic invalidation contract;
- [x] document listener/reconnect guarantees after moving the subscription adapter to `rustok-cache`.

## Verification

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- targeted runtime tests for backend selection, fallback, `load_or_fill` coalescing, invalidation validation/channel filtering, invalidation counters, Prometheus formatting and health semantics

## Update rules

1. When changing the cache backend contract, first update this file.
2. When changing the public/runtime contract, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.


## Quality backlog

- [x] Update test coverage for key module scenarios: added source-level tests for invalidation validation guardrails, channel-scoped local subscription parity, invalidation counters and Prometheus counter formatting without running compilation.
- [ ] Add compile/test evidence for the new invalidation coverage and the ignored real-Redis scenario when the compilation restriction is lifted.
- [x] Verify completeness and relevance of `README.md` and local docs: listener contract synchronized with Redis subscription validation and invalid payload rejection.
- [x] Lock/update verification gates for the current module state: added explicit ignored gate for real Redis pub/sub (`RUSTOK_CACHE_REAL_REDIS_URL=... cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture`).
