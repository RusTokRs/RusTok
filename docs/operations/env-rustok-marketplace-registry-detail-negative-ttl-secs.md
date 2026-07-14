---
id: doc://docs/operations/env-rustok-marketplace-registry-detail-negative-ttl-secs.md
kind: operations_documentation
language: en
source_language: en
entities:
  - env://RUSTOK_MARKETPLACE_REGISTRY_DETAIL_NEGATIVE_TTL_SECS
status: verified
---

# Environment Variable `RUSTOK_MARKETPLACE_REGISTRY_DETAIL_NEGATIVE_TTL_SECS`

## Purpose

Controls how briefly a missing or temporarily unavailable marketplace module detail is retained. Short negative caching coalesces repeated hot-slug misses without hiding a newly published module for the full positive catalog TTL.

## Contract

- Variable: `RUSTOK_MARKETPLACE_REGISTRY_DETAIL_NEGATIVE_TTL_SECS`
- Value: positive integer seconds
- Default: `5`
- The effective value is capped by `RUSTOK_MARKETPLACE_REGISTRY_CACHE_TTL_SECS`.
- Invalid, zero, or missing values fall back to the default.

## Operational guidance

Keep this TTL short. Raising it reduces registry traffic for repeated misses but increases the maximum delay before a newly available module detail becomes visible through an already running server instance.

## Evidence

- `apps/server/src/services/marketplace_catalog_cache.rs`
