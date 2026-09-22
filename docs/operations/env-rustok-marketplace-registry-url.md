---
id: doc://docs/operations/env-rustok-marketplace-registry-url.md
kind: operations_documentation
language: en
source_language: en
entities:
  - env://RUSTOK_MARKETPLACE_REGISTRIES
last_verified_snapshot: snap_jsonl_00000021
status: verified
---

# Environment Variable `RUSTOK_MARKETPLACE_REGISTRIES`

## Purpose

Document the runtime purpose, expected format, and default behavior for this environment variable.

## Contract

- Variable: `RUSTOK_MARKETPLACE_REGISTRIES`
- Canonical entity: `env://RUSTOK_MARKETPLACE_REGISTRIES`
- The value is optional. When absent, no remote provider is composed and the
  local manifest catalog remains available.
- When present, the value is a non-empty JSON array of objects with exact
  `id` and `url` fields:

  ```json
  [
    {
      "id": "community.eu",
      "url": "https://registry.eu.example/catalog"
    },
    {
      "id": "partners",
      "url": "https://partners.example/modules"
    }
  ]
  ```

- Every `id` must already be canonical lowercase and unique in the configured
  set. Registry identity is independent from the endpoint and participates in
  provider, cache, and release identity. Allowed characters are ASCII letters,
  digits, `.`, `_`, and `-`; the first and last characters must be
  alphanumeric.
- Every `url` must be an absolute HTTPS base URL.
- Embedded username/password credentials, query parameters, and fragments are
  rejected during startup. TLS certificate validation and redirect rejection
  remain enabled in the bounded registry client.
- A failed list fetch degrades the non-critical `marketplace_providers`
  readiness check while preserving the local catalog. A failed remote module
  detail transport/contract fetch returns an error rather than masquerading as
  not-found; an explicit remote `404` remains a valid not-found result.

## Evidence

- `apps/server/src/services/marketplace_catalog_cache_base.rs`
- `apps/server/src/controllers/health.rs`

## Notes

The array is the complete current remote-provider set. There is no implicit
default registry and no endpoint-derived identity.
