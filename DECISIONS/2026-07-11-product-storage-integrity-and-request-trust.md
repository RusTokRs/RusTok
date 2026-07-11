# Product storage integrity and request trust

- Date: 2026-07-11
- Status: Accepted

## Context

Product catalog storage is tenant-scoped and is consumed by admin, storefront,
inventory, taxonomy, pricing, and search. Product GraphQL write operations must
not trust tenant or actor identifiers supplied by a client. Earlier migrations
also allowed cross-tenant tag references, ambiguous primary categories, and
unconstrained EAV values. The product schema uses PostgreSQL features including
JSONB, partial indexes, enum types, and composite foreign keys.

## Decision

- `rustok-product` is PostgreSQL-only and fails migration execution on another
  backend.
- `TenantContext` and `AuthContext` are the only source of tenant and actor for
  product writes. Product GraphQL mutations expose neither identifier.
- `products.primary_category_id` is the canonical primary category. Product
  category assignments are navigation/collection/virtual bindings only.
- `catalog_categories` and its closure table are product-owned. Parent and
  closure references are tenant-composite, and the closure table remains the
  canonical ancestry projection.
- Product tags use tenant-composite foreign keys to product and taxonomy term
  identities. `rustok-taxonomy` owns the referenced tenant identity key.
- EAV scalar type, option ownership, and select cardinality are enforced in
  PostgreSQL in addition to service validation. Detached state is derived from
  the effective schema rather than independently persisted.

## Consequences

- Existing databases with duplicate root slugs, duplicate tenant handles/SKUs,
  multiple primary assignments, or cross-tenant tags must be remediated before
  the new constraints are applied.
- Public clients must use request-bound authentication and tenant routing for
  product writes; they cannot nominate a different actor or tenant.
- PostgreSQL migration lifecycle tests and query-plan evidence remain required
  before declaring the transport boundary verified.
