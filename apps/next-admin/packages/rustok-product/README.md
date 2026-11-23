# @rustok/product-admin

Module-owned Next.js administration package for the RusToK Product Catalog platform.

## Overview

This package provides the administration workspace for comprehensive product catalog management, variant matrices, category taxonomies, attribute schemas, and product bundles:

- **Products Workspace**: List, filter by status, search, and vendor; inspect and edit products with full multi-locale translations, SEO metadata, and media gallery.
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
canonicalized (or removed) before the table reads them. The Next catalog uses the canonical Product-owned `adminProductCatalog` GraphQL read.
Its server route accepts `category_id`, `attribute_filters`, `sort_by`, and
`sort_direction` query parameters in addition to the visible title/status/vendor filters,
matching the Leptos owner contract. The shared table receives the resolved page-size default
so its URL state and server result agree; scalar text filters stay scalar when restored from
the URL. Focused source
regressions in `src/product-list-pagination.test.mjs` and
`src/data-table-shell-pagination.test.mjs` are authored but unrun; no build or browser
behavior is certified.

## Admin catalog filter and facet contract (2026-11-22)

The dashboard product route now renders server-backed category, typed-attribute, and
sort controls plus value-count facets. Category choices are restricted to structural
categories; attribute choices are limited to filterable attributes with Product or Both
scope. Facet requests reuse the current search, status, vendor, category, and attribute
filters while deliberately omitting pagination, so counts describe the full filtered
catalog rather than only the visible page. Facet links reset the page to 1 and preserve
the selected page size and unrelated filters. The facet request is bounded to the first
eight filterable attribute codes. Option/facet transport failures degrade to empty filter
metadata without hiding an otherwise successful product list.

## Catalog URL reconciliation and selection identity (2026-11-23)

The table's URL-backed filter state is reconciled on browser Back/Forward, without
overwriting local edits on unrelated renders while a debounce is pending.
Product rows are keyed by owner Product IDs, so a checkbox selection from one
server page cannot be reassigned to a different Product on another page.
The Product table explicitly receives the server-normalized page size; category,
attribute and sort inputs remount with new URL-driven default values after
facet navigation. These source changes and their regressions have not been
runtime-verified.

## Facet selection consistency (2026-11-23; source only)

The Next catalog normalizes `code=value` whitespace at the URL read boundary,
matching the Product owner's parser, so a manually typed selection can be
recognized and removed with the counted facet toggle. The maximum eight
enumerated facets favor attributes with current selections before otherwise
eligible options, keeping their clear controls visible in large schemas.
Source regression guards are authored; runtime tests were not executed.

## Debounced filter navigation safety (2026-11-23; source only)

Browser Back/Forward invalidates pending column-filter debounces at the
`popstate` event, before the App Router's new server props commit. Ordered
signatures track overlapping local filter writes, with live nuqs values
overlaid on server route props so local state is recognized before the RSC
refresh. An older route response does not discard a newer edit, while a newer
committed route supersedes earlier writes. Source regression coverage is
authored; no runtime test or build has been run.
