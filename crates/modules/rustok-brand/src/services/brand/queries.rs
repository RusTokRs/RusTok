use sea_orm::{ColumnTrait, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::{
    dto::{BrandDto, BrandFilter, BrandListResponse},
    entities::{
        brand::{Column as BrandColumn, Entity as Brand},
        brand_product::{Column as BrandProductColumn, Entity as BrandProduct},
    },
    error::{BrandError, BrandResult},
};

use super::{
    BrandService,
    repository::{assemble_brand_dto, load_translations},
};

impl BrandService {
    pub(super) async fn get_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Brand ID cannot be nil".to_string(),
            ));
        }

        let brand = Brand::find_by_id(id)
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BrandError::NotFound(id))?;

        let translations = load_translations(&self.db, id).await?;

        Ok(assemble_brand_dto(brand, translations, locale))
    }

    pub(super) async fn get_brand_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        let clean_slug = slug.trim().to_lowercase();
        if clean_slug.is_empty() {
            return Err(BrandError::InvalidInput("Slug cannot be empty".to_string()));
        }

        let brand = Brand::find()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::Slug.eq(&clean_slug))
            .one(&self.db)
            .await?
            .ok_or_else(|| BrandError::SlugNotFound(slug.to_string()))?;

        let translations = load_translations(&self.db, brand.id).await?;

        Ok(assemble_brand_dto(brand, translations, locale))
    }

    pub(super) async fn list_brands(
        &self,
        tenant_id: Uuid,
        filter: BrandFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BrandResult<BrandListResponse> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        let page = if page == 0 { 1 } else { page };
        let per_page = if per_page == 0 {
            20
        } else {
            std::cmp::min(per_page, 100)
        };

        let mut query = Brand::find().filter(BrandColumn::TenantId.eq(tenant_id));

        if let Some(active) = filter.is_active {
            query = query.filter(BrandColumn::IsActive.eq(active));
        }

        if let Some(ref search) = filter.search {
            let s = search.trim();
            if !s.is_empty() {
                let bounded_s = &s[..s.floor_char_boundary(100)];
                query = query.filter(BrandColumn::Slug.contains(bounded_s));
            }
        }

        query = query.order_by(BrandColumn::Slug, Order::Asc);

        let paginator = query.paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        // An out-of-range page is empty. Do not pass an unbounded page
        // index to the paginator's offset multiplication.
        let brands = if page <= total.div_ceil(per_page) {
            paginator.fetch_page(page - 1).await?
        } else {
            Vec::new()
        };

        let mut dtos = Vec::with_capacity(brands.len());
        for brand in brands {
            let translations = load_translations(&self.db, brand.id).await?;
            dtos.push(assemble_brand_dto(brand, translations, locale));
        }

        Ok(BrandListResponse {
            items: dtos,
            total,
            page,
            per_page,
        })
    }

    pub(super) async fn get_brand_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if product_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Product ID cannot be nil".to_string(),
            ));
        }

        let link = BrandProduct::find()
            .filter(BrandProductColumn::TenantId.eq(tenant_id))
            .filter(BrandProductColumn::ProductId.eq(product_id))
            .order_by(BrandProductColumn::IsPrimary, Order::Desc)
            .one(&self.db)
            .await?;

        let Some(link) = link else {
            return Ok(None);
        };

        let brand = self.get_brand(tenant_id, link.brand_id, locale).await?;
        Ok(Some(brand))
    }
}
