---
id: doc://docs/architecture/global-registry-catalog-boundary.md
kind: architecture_decision
language: markdown
source_language: markdown
status: accepted
---
# Global Registry Catalog Boundary

## Decision

The read-only marketplace registry catalog is a deployment-global platform surface, not a tenant-owned domain surface.

The following routes intentionally bypass tenant resolution:

- `GET /catalog`;
- `GET /catalog/{slug}`.

These are the only current read-only catalog routes. Version-suffixed
compatibility aliases are not part of the platform contract.

Registry mutation and governance routes under `/v2/catalog/*` do not bypass tenant resolution.

## Rationale

The read-only catalog describes modules available to the deployed RusToK platform. Its source is the active platform composition manifest plus the global registry governance projection. It does not select tenant-owned catalog, product, order, customer or content records.

A registry-only deployment exposes the same read-only router so another RusToK deployment can discover platform modules without inventing a tenant identity for the registry service itself.

Locale may affect the global presentation projection through `RequestContext`, but locale is not a tenant ownership dimension and must not turn the registry catalog into a tenant-scoped data source.

## Isolation Contract

The bypass is permitted only while all of the following remain true:

1. the route is read-only;
2. the response is built from platform composition and global registry governance data;
3. no tenant-owned relation is queried or joined;
4. no tenant-specific enablement, pricing, entitlement, secret or configuration is returned;
5. mutation, publish, approval, yank, ownership and runner routes remain outside the bypass;
6. cache keys and ETags are based only on global catalog inputs and explicit presentation dimensions such as locale.

Any future tenant marketplace, tenant module enablement view or tenant-specific catalog overlay must use a different tenant-bound route and require `TenantContextExtension`.

## Enforcement Evidence

- `apps/server/src/controllers/marketplace_registry.rs::read_only_router` mounts only the two read-only catalog handlers.
- `apps/server/src/controllers/marketplace_registry.rs::router` adds `/v2/catalog/*` mutation and governance handlers separately.
- `apps/server/src/controllers/marketplace_registry.rs::first_party_catalog_modules` reads active platform composition and applies the global registry governance projection.
- `apps/server/src/middleware/tenant_tests.rs::bypasses_only_read_only_global_registry_catalog_routes` locks the route boundary and asserts that `/v2/catalog/*` is tenant-bound.

## Review Triggers

Review this decision before:

- adding a database query to the read-only catalog handlers;
- adding tenant-specific filtering or entitlement data;
- changing the bypass prefixes;
- exposing non-first-party private metadata;
- sharing catalog caches across different locale or authorization projections.
