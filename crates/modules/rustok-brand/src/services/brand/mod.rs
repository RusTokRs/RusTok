mod commands;
mod queries;
mod repository;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{
    dto::{BrandDto, BrandFilter, BrandListResponse, CreateBrandInput, UpdateBrandInput},
    error::BrandResult,
    ports::BrandPort,
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
