# Hybrid Brand Catalog and Product Binding Architecture

- Date: 2026-11-04
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-brand, rustok-product
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

The `rustok-brand` module was initially authored with an isolated persistence model for `brands`, `brand_translations`, and `brand_products`. However, as documented in `crates/modules/rustok-brand/docs/implementation-plan.md#product-ownership-review`, this introduced an unresolved ownership and integrity defect:
1. `brand_products` resided in `rustok-brand` with an unvalidated bare `product_id UUID` column lacking foreign key enforcement, tenant boundary validation against the product aggregate, or cascade cleanup when products are removed.
2. In parallel, `rustok-product` retained a historical unnormalized display string `products.vendor`, creating conflicting and ambiguous sources of truth for brand attribution.
3. Conventional e-commerce systems either reduce brands to flat attribute options (e.g. Magento 2 EAV attributes), which compromises rich multilingual brand storytelling, localized SEO, and media assets, or embed brands monolithically inside product tables, preventing standalone brand landing pages and cross-module re-use.

The platform requires an architecture that simultaneously satisfies:
- **Multilingual localization:** rich localized brand narratives, descriptions, names, and SEO metadata via parallel localized records;
- **First-class brand landing hubs:** dedicated `/brands` index and `/brands/:slug` landing pages with logo, hero banner, official links, and live catalog filtration;
- **Multi-branding and co-branding:** products associating with multiple brands (e.g. primary manufacturer plus collaborative partners) with deterministic primary attribution;
- **Relational and transactional integrity:** zero orphaned rows, tenant-scoped validation, and strict module boundary enforcement adhering to RusToK governance.

## Decision

Adopt the **Hybrid Architecture**: `rustok-brand` owns the brand master catalog and presentation, while `rustok-product` owns product-to-brand associations.

### 1. `rustok-brand` owns the Brand Master Catalog

`rustok-brand` is the sole canonical source of truth for:
- Brand identity (`brands` table: `id`, `tenant_id`, `slug`, `logo_media_id`, `banner_media_id`, `website_url`, `is_active`, `metadata`);
- Parallel localized content (`brand_translations` table: `id`, `brand_id`, `locale`, `name`, `description`, `meta_title`, `meta_description`);
- Brand-to-category scoping (`brand_categories` table: `id`, `tenant_id`, `brand_id REFERENCES brands(id) ON DELETE CASCADE`, `category_id`, `is_featured`, `created_at`);
- Unique slug constraints per tenant (`UNIQUE (tenant_id, slug)`);
- Transactional outbox publications for brand lifecycle events (`BrandCreated`, `BrandUpdated`, `BrandDeleted`);
- Owner query and command port `BrandPort` / `BrandReadPort`;
- Brand storefront routing: `/brands`, `/brands/:slug`, and category-scoped brand indexes `/:category_slug/brands`.

`rustok-brand` **MUST NOT** own product assignment tables (`brand_products` is deprecated and retired).

### 2. `rustok-product` owns Product-to-Brand Bindings

`rustok-product` is the sole canonical source of truth for product-to-brand associations via a dedicated `product_brands` table:
```sql
CREATE TABLE product_brands (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    brand_id UUID NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    role VARCHAR(32) NOT NULL DEFAULT 'manufacturer',
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_brands UNIQUE (tenant_id, product_id, brand_id)
);

CREATE UNIQUE INDEX uq_product_brands_primary 
    ON product_brands (tenant_id, product_id) 
    WHERE is_primary = TRUE;

CREATE INDEX idx_product_brands_lookup 
    ON product_brands (tenant_id, brand_id);
```

- When a product is created or updated, `rustok-product` validates all assigned `brand_id`s through `BrandReadPort::validate_brand_ids(tenant_id, &ids)`;
- When a product is deleted, PostgreSQL/SQLite foreign key cascading automatically purges associated bindings without orphaned records;
- Invariants enforce at most one `is_primary = true` brand per product;
- The legacy `products.vendor` field is retained strictly for backward compatibility and fallback display, but authoritative brand attribution resolves via `product_brands`.

### 3. Asynchronous Lifecycle and Event Coordination

- When a brand is deleted in `rustok-brand`, it emits `BrandDeleted { tenant_id, brand_id }` through the transactional outbox;
- An asynchronous consumer in `rustok-product` receives `BrandDeleted` and purges matching `product_brands` entries or rejects deletion if strict referential policies apply;
- Projections into `rustok-index` incorporate `primary_brand_id` and all associated `brand_ids` for facet filtering and search.

### 4. Dual-UI FFA Admin Parity

In accordance with `AGENTS.md` §2.1 (item 6):
- Leptos FFA Admin package in `crates/modules/rustok-brand/admin` maintains core brand catalog management;
- Next.js Admin package in `apps/next-admin/packages/brand` is introduced to achieve complete UI parity;
- The product editor in both Leptos and Next.js product admin surfaces includes a multi-brand selector with primary designation.

## Sources of truth and ownership

- **Canonical brand entity:** `rustok-brand::brands` and `rustok-brand::brand_translations`.
- **Canonical product-brand relation:** `rustok-product::product_brands`.
- **Derived search and catalog facets:** `rustok-index` product projection documents containing `brand_ids` and `primary_brand_id`.
- **Dependency direction:** `rustok-product` has an optional/pluggable read dependency on `rustok-brand` via `BrandReadPort`. `rustok-brand` has zero dependencies on `rustok-product`.

## Invariants

### Allowed states
- A brand has zero, one, or many translations across supported locales.
- A brand slug is unique per tenant.
- A product has zero, one, or many associated brands.
- A product with one or more associated brands has exactly one primary brand (`is_primary = true`).
- A product can associate with collaborative brands where `is_primary = false` and `role = 'collaboration'`.

### Forbidden states
- Multiple primary brands for the same product (`uq_product_brands_primary` violation).
- Assigning a `brand_id` that does not exist or belongs to another tenant.
- Cross-tenant brand associations.
- Orphan `product_brands` rows after a product is deleted (guaranteed by foreign key cascade).
- Writing brand modifications without an atomic transaction enclosing base row and translations.

## Non-goals

- Generic taxonomy replacement: brands are distinct from taxonomy categories and tags; they carry dedicated manufacturer metadata, logos, banners, and independent legal/contractual identities.
- EAV-attribute conversion: brands are not flattened into `product_attributes`.

## Data, transaction, and concurrency boundary

- Writes to `brands` and `brand_translations` are atomic in one owner transaction within `rustok-brand`.
- Writes to `products` and `product_brands` are atomic in one owner transaction within `rustok-product`.
- Optimistic locking and revision checks on `products.revision` guard concurrent product-brand modifications.

## Context dimensions

- **Tenant:** Mandatory on every table, index, query, mutation, and event payload.
- **Locale:** Used for parallel translations in `brand_translations` and storefront brand detail resolution.
- **Trace & Policy:** Permissions `brands:read`, `brands:write`, `products:update` enforced by callers before service execution.

## Events and projections

- `rustok-brand` publishes:
  - `BrandCreated { tenant_id, brand_id, slug }`
  - `BrandUpdated { tenant_id, brand_id, slug }`
  - `BrandDeleted { tenant_id, brand_id }`
- `rustok-product` listens to `BrandDeleted` to maintain referential hygiene.
- `rustok-index` updates product document facets when brand relations or brand localized names change.

## Failure semantics

- **Brand validation failure:** Product mutations attempting to associate non-existent or foreign-tenant `brand_id`s fail closed with `ProductValidationError::BrandNotFound` prior to transaction commit.
- **Primary brand conflict:** Attempting to assign more than one `is_primary = true` brand to a product violates `uq_product_brands_primary` and rolls back the command with a typed validation error.
- **Concurrent modifications:** Product-brand modifications require matching `expected_revision` against `products.revision`, rejecting concurrent conflicting writes with optimistic lock errors.
- **Asynchronous event delivery failure:** Outbox retries guarantee eventual cleanup of `product_brands` when a brand is deleted; read projections fail soft by omitting detached brand metadata.

## Migration and cutover

1. **Phase 1 (Expand):** Add migration `m20261104_create_product_brands` in `rustok-product` introducing the `product_brands` table and indexes.
2. **Phase 2 (Backfill):** Backfill existing assignments from `brand_products` (if present) into `product_brands`, mapping single associations to `is_primary = true`.
3. **Phase 3 (Port & Consumer Cutover):** Introduce `BrandReadPort` in `rustok-brand` for product-side brand resolution and validation. Update Commerce GraphQL and Product admin to use `product_brands`.
4. **Phase 4 (Contract):** Deprecate and drop `brand_products` in `rustok-brand`, removing `assign_product_brand` from `BrandPort`.
5. **Phase 5 (UI Parity):** Implement `@rustok/brand-admin` in `apps/next-admin/packages/brand` to achieve dual Leptos/Next.js admin parity.

## Alternatives considered

- **Brand as an EAV Attribute (Magento 2 model):** Rejected because flat attribute options do not natively support rich multilingual stories, media asset attachments (logos, hero banners), brand landing page routing, and primary/collaborative roles without complex secondary tables.
- **Complete Monolithic Merger into `rustok-product`:** Rejected because brands possess independent marketing lifecycles (landing hubs `/brands/:slug`, alphabetical brand directories, and future non-product integrations such as blog posts, sponsorships, and distributor profiles) that should not inflate the core product catalog aggregate.
- **Brand-owned Product Link (Status Quo):** Rejected due to lack of foreign key cascading, inability to prevent orphaned records upon product deletion, and violation of the aggregate root pattern where the Product aggregate must control its own attributes and relations.

## Verification

- `cargo test -p rustok-brand --lib --test brand`: validates brand master catalog CRUD, tenant isolation, and localized translation lifecycle.
- `cargo test -p rustok-product --lib`: validates `product_brands` persistence, tenant isolation, primary-brand uniqueness, and multi-brand associations.
- `cmd /c npm run verify:adrs`: validates ADR governance compliance.
- Scoped verification for affected GraphQL and UI packages upon phase completion.

## Consequences

### Positive
- Strict database-enforced relational integrity via foreign key cascades on product deletion.
- Clean domain separation: `rustok-brand` owns brand identity and presentation; `rustok-product` owns product merchandising and brand bindings.
- First-class support for co-branding (collaborations) with explicit roles and primary brand designation.
- Rich localized brand pages (`/brands/:slug`) with SEO, logos, banners, and faceted catalog integration.
- Elimination of orphaned association records and unvalidated foreign UUIDs.

### Negative / Trade-offs
- Requires cross-module outbox event coordination (`BrandDeleted`) for asynchronous referential cleanup when brands are removed.
- Introduces migration overhead to cut over existing `brand_products` consumers to `product_brands`.

