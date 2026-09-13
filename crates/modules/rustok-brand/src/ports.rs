use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{BrandDto, BrandFilter, BrandListResponse, CreateBrandInput, UpdateBrandInput};
use crate::error::BrandResult;

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

    async fn create_brand(
        &self,
        tenant_id: Uuid,
        input: CreateBrandInput,
    ) -> BrandResult<BrandDto>;

    async fn update_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBrandInput,
    ) -> BrandResult<BrandDto>;

    async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()>;

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
