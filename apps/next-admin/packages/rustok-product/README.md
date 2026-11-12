# @rustok/product-admin

Module-owned Next.js administration package for the RusToK Product Catalog platform.

## Overview

This package provides the administration workspace for comprehensive product catalog management, variant matrices, category taxonomies, attribute schemas, and product bundles:

- **Products Workspace**: List, filter (by status, search, vendor), inspect, and edit products with full multi-locale translations, SEO metadata, and media gallery.
- **Variants & Pricing**: Manage multi-axis product variants, SKU, barcode, inventory policy, exact major-unit decimal pricing (via `ExactDecimal` matching Rust's 96-bit decimal bounds), and explicit zero compare-at promotions.
- **Category Taxonomy**: Hierarchy of catalog categories with slugs, paths, codes, and multi-locale names.
- **Attribute Schemas**: Dynamic attribute authoring (text, integer, decimal, boolean, date, datetime, options, JSON) with filterable and sortable flags.
- **Product Bundles**: Create and manage bundle collections with individual item quantities and overrides.
- **Idempotent Writes & Journaling**: Built-in write journaling (`write-journal.ts`, `write-attempt.ts`) ensuring crash-resilient write attempts, lock serialization, and recovery against network timeouts.

## Architecture

- **Host Integration**: Registers `/dashboard/product`, `/dashboard/product/categories`, `/dashboard/product/attributes`, and `/dashboard/product/bundles` under `modulePlugins` via `@/modules/registry`.
- **Backend Boundary**: Communicates with the async-graphql catalog schema via `graphqlRequest` with tenant and session token propagation.
- **Shared FFA Primitives**: Renders data using `@/widgets/data-table`, `@/shared/ui/shadcn/*`, and Project Fluent translations.
