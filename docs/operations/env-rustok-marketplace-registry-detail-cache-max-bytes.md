---
id: doc://docs/operations/env-rustok-marketplace-registry-detail-cache-max-bytes.md
kind: operations_documentation
language: en
source_language: en
entities:
  - env://RUSTOK_MARKETPLACE_REGISTRY_DETAIL_CACHE_MAX_BYTES
status: verified
---

# Environment Variable `RUSTOK_MARKETPLACE_REGISTRY_DETAIL_CACHE_MAX_BYTES`

## Purpose

Bounds the total Moka weight retained by the registry marketplace module-detail cache. The weight includes the hashed cache key, fixed value metadata, module strings, version metadata, and serialized settings-schema bytes.

## Contract

- Variable: `RUSTOK_MARKETPLACE_REGISTRY_DETAIL_CACHE_MAX_BYTES`
- Value: positive integer byte count
- Default: `4194304` (4 MiB)
- Invalid, zero, or missing values fall back to the default.
- This budget is independent of `RUSTOK_MARKETPLACE_REGISTRY_CACHE_MAX_BYTES`, which bounds catalog-list responses.

## Operational guidance

Increase the budget only after observing module-detail payload distributions. Entries larger than the configured budget are not retained by the weighted cache, so correctness degrades to bounded registry fetches rather than unbounded memory growth.

## Evidence

- `apps/server/src/services/marketplace_catalog_cache.rs`
