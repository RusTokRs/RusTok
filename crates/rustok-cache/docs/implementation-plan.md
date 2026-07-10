# Implementation plan for `rustok-cache`

## Current state

`rustok-cache` is the capability-only core owner of backend selection,
in-memory fallback, Redis integration, invalidation, and anti-stampede loading.
The host consumes the cache contract; it must not distribute backend-specific
wiring or invalidation policy across unrelated modules.

The module provides `CacheService::load_or_fill`, Redis circuit-breaker options,
validated Redis pub/sub invalidation, channel-scoped local fan-out, and
service-level health, configuration, loader, and invalidation metrics. Invalid
channels and payloads are rejected before they reach local handlers.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `not_started`
- Structural shape: `no_ui_boundary`
- This capability has no module-owned UI or published FBA provider contract.

## Open results

1. **Run compiled cache contract coverage.** Execute the targeted unit suite
   for backend selection, fallback, `load_or_fill`, invalidation validation,
   metrics, and health semantics.
   **Depends on:** an environment where compilation is available.
   **Done when:** `cargo test -p rustok-cache --lib` validates the source-locked
   cache contract without skipped relevant coverage.

2. **Collect real Redis pub/sub evidence.** Run the ignored publisher/
   subscription scenario against a configured Redis instance and confirm the
   validated channel contract across instances.
   **Depends on:** `RUSTOK_CACHE_REAL_REDIS_URL` and an isolated Redis service.
   **Done when:** `real_redis_publish_and_subscription_share_validated_channel_contract`
   passes with observable publish, rejection, and local-fan-out behavior.

3. **Keep cache operational guidance synchronized.** Update the cache README
   and local documentation whenever backend modes, invalidation, reconnect, or
   metrics behavior changes.
   **Depends on:** the change-owning cache contract.
   **Done when:** callers can determine fallback behavior, observability
   signals, and recovery expectations from the module documentation.

## Verification

- `cargo xtask module validate cache`
- `cargo xtask module test cache`
- `cargo test -p rustok-cache --lib`
- `RUSTOK_CACHE_REAL_REDIS_URL=redis://... cargo test -p rustok-cache real_redis_publish_and_subscription_share_validated_channel_contract -- --ignored --nocapture`

## Change rules

1. Keep backend wiring, invalidation, and fallback policy in this module.
2. Update the root README, local docs, and `rustok-module.toml` with a cache
   contract change.
3. Update `docs/modules/registry.md` if module ownership or capability status
   changes.
