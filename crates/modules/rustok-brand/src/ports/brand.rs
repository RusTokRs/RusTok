use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{
    BrandCategoryDto, BrandDto, BrandFilter, BrandListResponse, CreateBrandInput, UpdateBrandInput,
};
use crate::error::BrandResult;

/// Owner contract for brand operations.
///
/// Transport callers resolve the effective tenant and enforce the applicable
/// product permission before calling the service. The service scopes owner
/// rows to that tenant; a bare tenant ID is not authorization.
#[async_trait]
pub trait BrandPort: Send + Sync {
    async fn get_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto>;

    async fn get_brand_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto>;

    async fn list_brands(
        &self,
        tenant_id: Uuid,
        filter: BrandFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BrandResult<BrandListResponse>;

    async fn create_brand(&self, tenant_id: Uuid, input: CreateBrandInput)
    -> BrandResult<BrandDto>;

    async fn update_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBrandInput,
    ) -> BrandResult<BrandDto>;

    async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()>;

    // Category scoping
    async fn assign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
        is_featured: bool,
    ) -> BrandResult<()>;

    async fn unassign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
    ) -> BrandResult<()>;

    async fn list_categories_for_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
    ) -> BrandResult<Vec<BrandCategoryDto>>;

    // Product associations (transitional during hybrid cutover)
    async fn get_brand_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>>;

    async fn assign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
        is_primary: bool,
    ) -> BrandResult<()>;

    async fn unassign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
    ) -> BrandResult<()>;
}

/// Cross-boundary read port consumed by other modules (such as rustok-product).
#[async_trait]
pub trait BrandReadPort: Send + Sync {
    /// Validates a list of brand IDs against the specified tenant, returning
    /// only the IDs that exist and are active.
    async fn validate_brand_ids(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
    ) -> BrandResult<Vec<Uuid>>;

    /// Retrieves a single brand by ID with optional localized text.
    async fn get_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>>;

    /// Batch-fetches multiple brands by their IDs for efficient projection.
    async fn get_brands_batch(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>>;

    /// Lists active brands scoped to a specific category.
    async fn list_brands_for_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>>;
}
