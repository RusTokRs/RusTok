# CRATE_API: rustok-brand

Canonical public contract and domain architecture specification for `rustok-brand`.

---

## Public Modules
- `dto`: Input and output data transfer objects (`BrandDto`, `BrandCategoryDto`, `CreateBrandInput`, `UpdateBrandInput`, `BrandFilter`).
- `error`: Strongly typed domain error variants (`BrandError`, `BrandResult`).
- `migrations`: SeaORM migration chain for brand schemas.
- `module`: Lifecycle descriptor `BrandModule` implementing `RusToKModule`.
- `ports`: Owner-defined cross-boundary port `BrandPort` and `BrandReadPort`.
- `services`: Domain service layer (`BrandService`).

## Platform Capability Bindings

- **Module Identifier:** `brand`
- **Module Name:** `Brand`
- **Module Kind:** `ModuleKind::Optional`
- **Crate Version:** `0.1.0` (workspace)
- **Permissions:**
  - `Permission::PRODUCTS_READ`
  - `Permission::PRODUCTS_UPDATE`
- **Migration Source:**
  - `m20260915_000001_create_brands`
  - `m20261104_000002_create_brand_categories`
  - Upstream dependency: `m20250130_000012_create_commerce_products`

---

## Domain Collaborations

- **Upstream Modules:**
  - `rustok-core`: Common module lifecycle traits (`RusToKModule`, `MigrationSource`, `ModuleKind`, `ModuleHealth`).
  - `rustok-api`: Standard permission definitions (`Permission`).
- **Downstream Consumers:**
  - `rustok-product`: Consumes `BrandReadPort` to validate and batch-load brand metadata.
  - `rustok-commerce`: Exposes brand queries and mutations via GraphQL.
- **Persistence Layer:**
  - SeaORM entities: `brand`, `brand_translation`, `brand_category`, `brand_product`.
  - Multi-tenant tenant scoping across all tables.

---

## Minimum Contract Set

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

      // Category scoping
      async fn assign_brand_category(&self, tenant_id: Uuid, brand_id: Uuid, category_id: Uuid, is_featured: bool) -> BrandResult<()>;
      async fn unassign_brand_category(&self, tenant_id: Uuid, brand_id: Uuid, category_id: Uuid) -> BrandResult<()>;
      async fn list_categories_for_brand(&self, tenant_id: Uuid, brand_id: Uuid) -> BrandResult<Vec<BrandCategoryDto>>;

      // Product associations (transitional during hybrid cutover)
      async fn get_brand_for_product(&self, tenant_id: Uuid, product_id: Uuid, locale: Option<&str>) -> BrandResult<Option<BrandDto>>;
      async fn assign_product_brand(&self, tenant_id: Uuid, brand_id: Uuid, product_id: Uuid, is_primary: bool) -> BrandResult<()>;
      async fn unassign_product_brand(&self, tenant_id: Uuid, brand_id: Uuid, product_id: Uuid) -> BrandResult<()>;
  }
  ```

- **`BrandReadPort` Trait:**
  ```rust
  #[async_trait]
  pub trait BrandReadPort: Send + Sync {
      async fn validate_brand_ids(&self, tenant_id: Uuid, brand_ids: &[Uuid]) -> BrandResult<Vec<Uuid>>;
      async fn get_brand(&self, tenant_id: Uuid, id: Uuid, locale: Option<&str>) -> BrandResult<Option<BrandDto>>;
      async fn get_brands_batch(&self, tenant_id: Uuid, brand_ids: &[Uuid], locale: Option<&str>) -> BrandResult<Vec<BrandDto>>;
      async fn list_brands_for_category(&self, tenant_id: Uuid, category_id: Uuid, locale: Option<&str>) -> BrandResult<Vec<BrandDto>>;
  }
  ```

---

### Input DTOs/Commands

- `CreateBrandInput`: Slug (trimmed/lowercased), optional logo and banner media UUIDs, website URL, active status flag, metadata JSON, and localized translations (`BrandTranslationInput`).
- `UpdateBrandInput`: Optional replacement fields for slug, logo, banner, website, active status, metadata, and localized translations.
- `BrandFilter`: Optional search string (matching slug), active status filter, and category scoping filter (`category_id`).
- `BrandDto`: Brand projection including localized name, description, media IDs, website URL, and translation records.
- `BrandCategoryDto`: Relational mapping between brand and category with `is_featured` flag.
- `BrandTranslationInput` / `BrandTranslationDto`: Localized content (locale, name, description).
- `BrandProductDto`: Relational mapping between brand and product with `is_primary` flag.

---

### Domain Invariants

- **Tenant Scoping:** Brand reads and writes scope base rows and associations by `tenant_id`. Translation rows are scoped through their owning brand.
- **Slug Uniqueness:** Brand slugs must be unique within a given tenant. Slugs are trimmed and normalized to lowercase.
- **Slug Non-Empty:** Empty or whitespace-only slugs are strictly rejected with `BrandError::InvalidInput`.
- **Locale Resolution Fallback:** When a specific locale is requested, translation lookup matches requested locale first, then falls back to English (`en*`), then the first available translation, or defaults to the brand slug if no translation exists.
- **Category Scoping:** Brands can be assigned to multiple categories (M:N) via `brand_categories`. When querying brands for a category, featured brands precede standard brands.

---

### Events / Outbox Side Effects

- Brand mutations (creation, update, deletion) are atomically transacted in PostgreSQL/SQLite.

---

### Errors / Failure Codes

- `BrandError::NotFound(Uuid)`: Brand not found by UUID for the given tenant.
- `BrandError::SlugNotFound(String)`: Brand not found by slug for the given tenant.
- `BrandError::SlugAlreadyExists(String)`: Duplicate slug within the same tenant.
- `BrandError::InvalidInput(String)`: Invalid input payload (empty slug, invalid values).
- `BrandError::Database(DbErr)`: Database query or transaction failure.
- `BrandError::Outbox(String)`: Outbox dispatch or publishing failure.
