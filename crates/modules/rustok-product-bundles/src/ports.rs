use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{
    BundleDto, BundleFilter, BundleItemDto, BundleItemInput, BundleListResponse, CreateBundleInput,
    UpdateBundleInput,
};
use crate::error::BundleResult;

#[async_trait]
pub trait BundlePort: Send + Sync {
    async fn get_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto>;

    async fn get_bundle_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto>;

    async fn list_bundles(
        &self,
        tenant_id: Uuid,
        filter: BundleFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BundleResult<BundleListResponse>;

    async fn create_bundle(
        &self,
        tenant_id: Uuid,
        input: CreateBundleInput,
    ) -> BundleResult<BundleDto>;

    async fn update_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBundleInput,
    ) -> BundleResult<BundleDto>;

    async fn delete_bundle(&self, tenant_id: Uuid, id: Uuid) -> BundleResult<()>;

    async fn add_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> BundleResult<BundleItemDto>;

    async fn remove_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item_id: Uuid,
    ) -> BundleResult<()>;

    async fn get_bundles_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<Vec<BundleDto>>;
}
