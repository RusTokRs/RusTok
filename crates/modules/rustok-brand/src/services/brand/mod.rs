mod commands;
mod queries;
mod repository;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    dto::{
        BrandCategoryDto, BrandDto, BrandFilter, BrandListResponse, CreateBrandInput,
        UpdateBrandInput,
    },
    error::BrandResult,
    ports::{BrandPort, BrandReadPort},
};

#[derive(Clone)]
pub struct BrandService {
    db: DatabaseConnection,
}

impl BrandService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl BrandPort for BrandService {
    async fn get_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto> {
        Self::get_brand(self, tenant_id, id, locale).await
    }

    async fn get_brand_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto> {
        Self::get_brand_by_slug(self, tenant_id, slug, locale).await
    }

    async fn list_brands(
        &self,
        tenant_id: Uuid,
        filter: BrandFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BrandResult<BrandListResponse> {
        Self::list_brands(self, tenant_id, filter, page, per_page, locale).await
    }

    async fn create_brand(
        &self,
        tenant_id: Uuid,
        input: CreateBrandInput,
    ) -> BrandResult<BrandDto> {
        Self::create_brand(self, tenant_id, input).await
    }

    async fn update_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBrandInput,
    ) -> BrandResult<BrandDto> {
        Self::update_brand(self, tenant_id, id, input).await
    }

    async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()> {
        Self::delete_brand(self, tenant_id, id).await
    }

    async fn assign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
        is_featured: bool,
    ) -> BrandResult<()> {
        Self::assign_brand_category(self, tenant_id, brand_id, category_id, is_featured).await
    }

    async fn unassign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
    ) -> BrandResult<()> {
        Self::unassign_brand_category(self, tenant_id, brand_id, category_id).await
    }

    async fn list_categories_for_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
    ) -> BrandResult<Vec<BrandCategoryDto>> {
        Self::list_categories_for_brand(self, tenant_id, brand_id).await
    }

    async fn get_brand_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>> {
        Self::get_brand_for_product(self, tenant_id, product_id, locale).await
    }

    async fn assign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
        is_primary: bool,
    ) -> BrandResult<()> {
        Self::assign_product_brand(self, tenant_id, brand_id, product_id, is_primary).await
    }

    async fn unassign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
    ) -> BrandResult<()> {
        Self::unassign_product_brand(self, tenant_id, brand_id, product_id).await
    }
}

#[async_trait]
impl BrandReadPort for BrandService {
    async fn validate_brand_ids(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
    ) -> BrandResult<Vec<Uuid>> {
        Self::validate_brand_ids(self, tenant_id, brand_ids).await
    }

    async fn get_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>> {
        match Self::get_brand(self, tenant_id, id, locale).await {
            Ok(b) => Ok(Some(b)),
            Err(crate::error::BrandError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn get_brands_batch(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>> {
        Self::get_brands_batch(self, tenant_id, brand_ids, locale).await
    }

    async fn list_brands_for_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>> {
        Self::list_brands_for_category(self, tenant_id, category_id, locale).await
    }
}
