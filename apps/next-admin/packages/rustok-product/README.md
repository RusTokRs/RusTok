# @rustok/product-admin

Module-owned Next.js administration package for the RusToK Product Catalog platform.

## Overview

This package provides the administration workspace for comprehensive product catalog management, variant matrices, category taxonomies, attribute schemas, and product bundles:

- **Products Workspace**: List, filter (by status, search, vendor, category, and typed attribute filters), inspect, and edit products with full multi-locale translations, SEO metadata, and media gallery.
- **Variants & Pricing**: Manage multi-axis product variants, SKU, barcode, inventory policy, exact major-unit decimal pricing (via `ExactDecimal` matching Rust's 96-bit decimal bounds), and explicit zero compare-at promotions.
- **Category Taxonomy**: Hierarchy of catalog categories with slugs, paths, codes, and multi-locale names.
- **Attribute Schemas**: Dynamic attribute authoring (text, integer, decimal, boolean, date, datetime, options, JSON) with filterable and sortable flags.
- **Product Bundles**: Create and manage bundle collections with individual item quantities and overrides.
- **Idempotent Writes & Journaling**: Built-in write journaling (`write-journal.ts`, `write-attempt.ts`) ensuring crash-resilient write attempts, lock serialization, and recovery against network timeouts.

## Architecture

- **Host Integration**: Registers `/dashboard/product`, `/dashboard/product/categories`, `/dashboard/product/attributes`, and `/dashboard/product/bundles` under `modulePlugins` via `@/modules/registry`.
- **Backend Boundary**: Communicates with the async-graphql catalog schema via `graphqlRequest` with tenant and session token propagation.
- **Shared FFA Primitives**: Renders data using `@/widgets/data-table`, `@/shared/ui/shadcn/*`, and Project Fluent translations.

## Catalog pagination input boundary (2026-11-22)

The Product-owned `normalizeProductAdminPagination` utility clamps both server-route and
transport pagination to the Product read contract (page 1..2,147,483,647 and page size
1..100, default 20). Invalid URLs are canonicalized before the shared Next table reads
`page`/`perPage`, preventing the URL-backed table from displaying a different page
size than the GraphQL result. Direct module `listProducts` callers apply the same
bounds. The status picker uses the Product owner's single-status filter and columns do not offer
sorting that the server query does not implement. Existing invalid status deep links are
canonicalized (or removed) before the table reads them. The Next catalog uses the canonical Product-owned `adminProductCatalog` GraphQL read and
forwards the same `category_id`, `attribute_filters`, `sort_by`, and `sort_direction`
route filters as the Leptos catalog. The vendor filter is part of the same owner query.
The shared table now receives the resolved page-size default so its URL state and server
result agree; scalar text filters stay scalar when restored from the URL. Focused source
regressions in `src/product-list-pagination.test.mjs` and
`src/data-table-shell-pagination.test.mjs` are authored but unrun; no build or browser
behavior is certified.
