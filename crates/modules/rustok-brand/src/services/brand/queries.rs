use sea_orm::{ColumnTrait, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::{
    dto::{BrandCategoryDto, BrandDto, BrandFilter, BrandListResponse},
    entities::{
        brand::{Column as BrandColumn, Entity as Brand},
        brand_category::{Column as BrandCategoryColumn, Entity as BrandCategory},
        brand_product::{Column as BrandProductColumn, Entity as BrandProduct},
    },
    error::{BrandError, BrandResult},
};

use super::{
    BrandService,
    repository::{assemble_brand_dto, load_translations, load_translations_batch},
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

        if let Some(cat_id) = filter.category_id {
            let brand_ids: Vec<Uuid> = BrandCategory::find()
                .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
                .filter(BrandCategoryColumn::CategoryId.eq(cat_id))
                .all(&self.db)
                .await?
                .into_iter()
                .map(|bc| bc.brand_id)
                .collect();
            query = query.filter(BrandColumn::Id.is_in(brand_ids));
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

        let brand_ids: Vec<Uuid> = brands.iter().map(|b| b.id).collect();
        let mut translations_by_brand =
            load_translations_batch(&self.db, &brand_ids).await?;

        let mut dtos = Vec::with_capacity(brands.len());
        for brand in brands {
            let translations = translations_by_brand.remove(&brand.id).unwrap_or_default();
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

    pub(super) async fn list_categories_for_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
    ) -> BrandResult<Vec<BrandCategoryDto>> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if brand_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Brand ID cannot be nil".to_string(),
            ));
        }

        let links = BrandCategory::find()
            .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
            .filter(BrandCategoryColumn::BrandId.eq(brand_id))
            .all(&self.db)
            .await?;

        Ok(links
            .into_iter()
            .map(|m| BrandCategoryDto {
                id: m.id,
                tenant_id: m.tenant_id,
                brand_id: m.brand_id,
                category_id: m.category_id,
                is_featured: m.is_featured,
                created_at: m.created_at.into(),
            })
            .collect())
    }

    pub(super) async fn validate_brand_ids(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
    ) -> BrandResult<Vec<Uuid>> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if brand_ids.is_empty() {
            return Ok(Vec::new());
        }

        let valid_rows = Brand::find()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::IsActive.eq(true))
            .filter(BrandColumn::Id.is_in(brand_ids.to_vec()))
            .all(&self.db)
            .await?;

        Ok(valid_rows.into_iter().map(|b| b.id).collect())
    }

    pub(super) async fn get_brands_batch(
        &self,
        tenant_id: Uuid,
        brand_ids: &[Uuid],
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if brand_ids.is_empty() {
            return Ok(Vec::new());
        }

        let brands = Brand::find()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::Id.is_in(brand_ids.to_vec()))
            .all(&self.db)
            .await?;

        let brand_ids_vec: Vec<Uuid> = brands.iter().map(|b| b.id).collect();
        let mut translations_by_brand =
            load_translations_batch(&self.db, &brand_ids_vec).await?;

        let mut map: std::collections::HashMap<Uuid, BrandDto> =
            std::collections::HashMap::with_capacity(brands.len());
        for brand in brands {
            let translations = translations_by_brand.remove(&brand.id).unwrap_or_default();
            let dto = assemble_brand_dto(brand, translations, locale);
            map.insert(dto.id, dto);
        }

        let ordered = brand_ids.iter().filter_map(|id| map.remove(id)).collect();

        Ok(ordered)
    }

    pub(super) async fn list_brands_for_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Vec<BrandDto>> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        if category_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Category ID cannot be nil".to_string(),
            ));
        }

        let links = BrandCategory::find()
            .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
            .filter(BrandCategoryColumn::CategoryId.eq(category_id))
            .order_by(BrandCategoryColumn::IsFeatured, Order::Desc)
            .all(&self.db)
            .await?;

        let brand_ids: Vec<Uuid> = links.into_iter().map(|l| l.brand_id).collect();
        self.get_brands_batch(tenant_id, &brand_ids, locale).await
    }
}
