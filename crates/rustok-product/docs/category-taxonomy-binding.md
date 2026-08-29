# Product Category → Taxonomy binding seam

Status: **source-complete additive seam; backfill and runtime cutover pending**

TAXONOMY-CAT-23 introduces the Product-owned binding required before Product
Category identity, hierarchy and localized canonical copy can move to Taxonomy.
This slice is intentionally additive and data-preserving.

## Storage contract

`product_catalog_category_taxonomy_bindings` is Product-owned relation storage.
Each row contains:

- `tenant_id`;
- `catalog_category_id`, referencing Product `catalog_categories(tenant_id, id)`;
- `taxonomy_category_id`, referencing `taxonomy_terms(tenant_id, id)`;
- `created_at`.

The Product category identity is unique per tenant in the binding table and one
Taxonomy category may bind to at most one Product catalog category per tenant.
Cross-tenant bindings are rejected by composite foreign keys.

The physical binding table is currently created only on PostgreSQL because the
retained Product `(tenant_id, id)` category identity prerequisite is installed
by the PostgreSQL-only tenant-consistency migration. Other backends remain a
no-op for this seam until they have an equivalent tenant-safe prerequisite.

## Current cutover boundary

This slice does **not** backfill the binding and does **not** switch Product
reads or writes. Product still owns the live `catalog_categories`,
`catalog_category_translations`, hierarchy/closure, category-bound attribute
schema, product/category membership and virtual-category behavior until later
verified slices migrate those responsibilities deliberately.

The next migration slice must deterministically backfill Taxonomy Category rows
while preserving Product category UUIDs where possible, localized copy and
hierarchy. It must fail closed on incompatible Taxonomy identity/route
collisions and populate this binding before any Product runtime cutover.

No `product/category` Translation provider should be introduced during the
migration. Canonical Category Translation ownership after cutover is the
registered Taxonomy `taxonomy/term` provider.
