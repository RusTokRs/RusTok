# CRATE_API: rustok-brand

Canonical public contract and domain architecture specification for `rustok-brand`.

---

## 1. Platform Capability Bindings

- **Module Identifier:** `brand`
- **Module Name:** `Brand`
- **Module Kind:** `ModuleKind::Optional`
- **Crate Version:** `0.1.0` (workspace)
- **Permissions:**
  - `Permission::PRODUCTS_READ`
  - `Permission::PRODUCTS_UPDATE`
- **Migration Source:**
  - `m20260915_000001_create_brands`
  - Upstream dependency: `m20250130_000012_create_commerce_products`

---

## 2. Domain Collaborations

- **Upstream Modules:**
  - `rustok-core`: Common module lifecycle traits (`RusToKModule`, `MigrationSource`, `ModuleKind`, `ModuleHealth`).
  - `rustok-api`: Standard permission definitions (`Permission`).
  - `rustok-product`: Product entity references for brand-to-product mapping.
- **Persistence Layer:**
  - SeaORM entities: `brand`, `brand_translation`, `brand_product`.
  - Multi-tenant tenant scoping across all tables.

---

## 3. Minimum Contract Set

- **`BrandPort` Trait:**
  ```rust
  #[async_trait]
  pub trait BrandPort: Send + Sync {
      async fn get_brand(&self, tenant_id: Uuid, id: Uuid, locale: Option<&str>) -> BrandResult<BrandDto>;
      async fn get_brand_by_slug(&self, tenant_id: Uuid, slug: &str, locale: Option<&str>) -> BrandResult<BrandDto>;
      async fn list_brands(&self, tenant_id: Uuid, filter: BrandFilter, page: u64, per_page: u64, locale: Option<&str>) -> BrandResult<BrandListResponse>;
      async fn create_brand(&self, tenant_id: Uuid, input: CreateBrandInput) -> BrandResult<BrandDto>;
      async fn update_brand(&self, tenant_id: Uuid, id: Uuid, input: UpdateBrandInput) -> BrandResult<BrandDto>;
      async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()>;
      async fn get_brand_for_product(&self, tenant_id: Uuid, product_id: Uuid, locale: Option<&str>) -> BrandResult<Option<BrandDto>>;
      async fn assign_product_brand(&self, tenant_id: Uuid, brand_id: Uuid, product_id: Uuid, is_primary: bool) -> BrandResult<()>;
      async fn unassign_product_brand(&self, tenant_id: Uuid, brand_id: Uuid, product_id: Uuid) -> BrandResult<()>;
  }
  ```

---

## 4. Input DTOs / Commands

- `CreateBrandInput`: Slug (trimmed/lowercased), optional logo and banner media UUIDs, website URL, active status flag, metadata JSON, and localized translations (`BrandTranslationInput`).
- `UpdateBrandInput`: Optional replacement fields for slug, logo, banner, website, active status, metadata, and localized translations.
- `BrandFilter`: Optional search string (matching slug) and active status filter.
- `BrandDto`: Brand projection including localized name, description, media IDs, website URL, and translation records.
- `BrandTranslationInput` / `BrandTranslationDto`: Localized content (locale, name, description).
- `BrandProductDto`: Relational mapping between brand and product with `is_primary` flag.

---

## 5. Domain Invariants

- **Tenant Scoping:** Every brand, translation, and brand-product association is partitioned by `tenant_id`.
- **Slug Uniqueness:** Brand slugs must be unique within a given tenant. Slugs are trimmed and normalized to lowercase.
- **Slug Non-Empty:** Empty or whitespace-only slugs are strictly rejected with `BrandError::InvalidInput`.
- **Locale Resolution Fallback:** When a specific locale is requested, translation lookup matches requested locale first, then falls back to English (`en*`), then the first available translation, or defaults to the brand slug if no translation exists.
- **Primary Brand Relation:** A product can have associated brands, ordered by `is_primary` descending.

---

## 6. Events / Outbox Side Effects

- Brand mutations (creation, update, deletion) are atomically transacted in PostgreSQL/SQLite.
- Outbox integration reserved for brand publication and catalog index synchronization events.

---

## 7. Errors / Failure Codes

- `BrandError::NotFound(Uuid)`: Brand not found by UUID for the given tenant.
- `BrandError::SlugNotFound(String)`: Brand not found by slug for the given tenant.
- `BrandError::SlugAlreadyExists(String)`: Duplicate slug within the same tenant.
- `BrandError::InvalidInput(String)`: Invalid input payload (empty slug, invalid values).
- `BrandError::Database(DbErr)`: Database query or transaction failure.
- `BrandError::Outbox(String)`: Outbox dispatch or publishing failure.
