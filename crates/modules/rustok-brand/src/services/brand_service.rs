use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    BrandDto, BrandFilter, BrandListResponse, BrandTranslationDto, CreateBrandInput,
    UpdateBrandInput,
};
use crate::entities::{
    brand::{ActiveModel as BrandActiveModel, Column as BrandColumn, Entity as Brand},
    brand_product::{
        ActiveModel as BrandProductActiveModel, Column as BrandProductColumn,
        Entity as BrandProduct,
    },
    brand_translation::{
        ActiveModel as BrandTranslationActiveModel, Column as BrandTranslationColumn,
        Entity as BrandTranslation,
    },
};
use crate::error::{BrandError, BrandResult};
use crate::ports::BrandPort;

#[derive(Clone)]
pub struct BrandService {
    db: DatabaseConnection,
}

impl BrandService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn assemble_brand_dto(
        model: crate::entities::brand::Model,
        translations: Vec<crate::entities::brand_translation::Model>,
        locale: Option<&str>,
    ) -> BrandDto {
        let translation_dtos: Vec<BrandTranslationDto> = translations
            .iter()
            .map(|t| BrandTranslationDto {
                id: t.id,
                locale: t.locale.clone(),
                name: t.name.clone(),
                description: t.description.clone(),
            })
            .collect();

        // Best match for requested locale
        let effective_trans = if let Some(req_loc) = locale {
            translations
                .iter()
                .find(|t| t.locale.eq_ignore_ascii_case(req_loc))
                .or_else(|| translations.iter().find(|t| t.locale.starts_with("en")))
                .or_else(|| translations.first())
        } else {
            translations
                .iter()
                .find(|t| t.locale.starts_with("en"))
                .or_else(|| translations.first())
        };

        let name = effective_trans
            .map(|t| t.name.clone())
            .unwrap_or_else(|| model.slug.clone());
        let description = effective_trans.and_then(|t| t.description.clone());

        BrandDto {
            id: model.id,
            tenant_id: model.tenant_id,
            slug: model.slug,
            name,
            description,
            logo_media_id: model.logo_media_id,
            banner_media_id: model.banner_media_id,
            website_url: model.website_url,
            is_active: model.is_active,
            metadata: model.metadata,
            translations: translation_dtos,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
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
        let brand = Brand::find_by_id(id)
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BrandError::NotFound(id))?;

        let translations = BrandTranslation::find()
            .filter(BrandTranslationColumn::BrandId.eq(id))
            .all(&self.db)
            .await?;

        Ok(Self::assemble_brand_dto(brand, translations, locale))
    }

    async fn get_brand_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BrandResult<BrandDto> {
        let brand = Brand::find()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::Slug.eq(slug))
            .one(&self.db)
            .await?
            .ok_or_else(|| BrandError::SlugNotFound(slug.to_string()))?;

        let translations = BrandTranslation::find()
            .filter(BrandTranslationColumn::BrandId.eq(brand.id))
            .all(&self.db)
            .await?;

        Ok(Self::assemble_brand_dto(brand, translations, locale))
    }

    async fn list_brands(
        &self,
        tenant_id: Uuid,
        filter: BrandFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BrandResult<BrandListResponse> {
        let page = if page == 0 { 1 } else { page };
        let per_page = if per_page == 0 { 20 } else { per_page };

        let mut query = Brand::find().filter(BrandColumn::TenantId.eq(tenant_id));

        if let Some(active) = filter.is_active {
            query = query.filter(BrandColumn::IsActive.eq(active));
        }

        if let Some(ref search) = filter.search {
            let s = search.trim();
            if !s.is_empty() {
                query = query.filter(BrandColumn::Slug.contains(s));
            }
        }

        query = query.order_by(BrandColumn::Slug, Order::Asc);

        let paginator = query.paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let brands = paginator.fetch_page(page - 1).await?;

        let mut dtos = Vec::with_capacity(brands.len());
        for brand in brands {
            let translations = BrandTranslation::find()
                .filter(BrandTranslationColumn::BrandId.eq(brand.id))
                .all(&self.db)
                .await?;
            dtos.push(Self::assemble_brand_dto(brand, translations, locale));
        }

        Ok(BrandListResponse {
            items: dtos,
            total,
            page,
            per_page,
        })
    }

    async fn create_brand(
        &self,
        tenant_id: Uuid,
        input: CreateBrandInput,
    ) -> BrandResult<BrandDto> {
        let slug = input.slug.trim().to_lowercase();
        if slug.is_empty() {
            return Err(BrandError::InvalidInput("Slug cannot be empty".to_string()));
        }

        // Verify uniqueness
        let existing = Brand::find()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::Slug.eq(&slug))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(BrandError::SlugAlreadyExists(slug));
        }

        let brand_id = Uuid::new_v4();
        let now = Utc::now();

        let txn = self.db.begin().await?;

        let brand_model = BrandActiveModel {
            id: Set(brand_id),
            tenant_id: Set(tenant_id),
            slug: Set(slug),
            logo_media_id: Set(input.logo_media_id),
            banner_media_id: Set(input.banner_media_id),
            website_url: Set(input.website_url),
            is_active: Set(input.is_active.unwrap_or(true)),
            metadata: Set(input.metadata.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        let mut trans_models = Vec::new();
        for t in input.translations {
            let trans_model = BrandTranslationActiveModel {
                id: Set(Uuid::new_v4()),
                brand_id: Set(brand_id),
                locale: Set(t.locale.to_lowercase()),
                name: Set(t.name),
                description: Set(t.description),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
            trans_models.push(trans_model);
        }

        txn.commit().await?;

        Ok(Self::assemble_brand_dto(brand_model, trans_models, None))
    }

    async fn update_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBrandInput,
    ) -> BrandResult<BrandDto> {
        let brand = Brand::find_by_id(id)
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BrandError::NotFound(id))?;

        let txn = self.db.begin().await?;

        let mut active: BrandActiveModel = brand.into();
        let now = Utc::now();
        active.updated_at = Set(now.into());

        if let Some(slug) = input.slug {
            let clean_slug = slug.trim().to_lowercase();
            if clean_slug.is_empty() {
                return Err(BrandError::InvalidInput("Slug cannot be empty".to_string()));
            }
            let duplicate = Brand::find()
                .filter(BrandColumn::TenantId.eq(tenant_id))
                .filter(BrandColumn::Slug.eq(&clean_slug))
                .filter(BrandColumn::Id.ne(id))
                .one(&txn)
                .await?;
            if duplicate.is_some() {
                return Err(BrandError::SlugAlreadyExists(clean_slug));
            }
            active.slug = Set(clean_slug);
        }

        if let Some(logo) = input.logo_media_id {
            active.logo_media_id = Set(logo);
        }
        if let Some(banner) = input.banner_media_id {
            active.banner_media_id = Set(banner);
        }
        if let Some(url) = input.website_url {
            active.website_url = Set(url);
        }
        if let Some(is_active) = input.is_active {
            active.is_active = Set(is_active);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }

        let updated_brand = active.update(&txn).await?;

        if let Some(translations) = input.translations {
            for t in translations {
                let loc = t.locale.to_lowercase();
                let existing_trans = BrandTranslation::find()
                    .filter(BrandTranslationColumn::BrandId.eq(id))
                    .filter(BrandTranslationColumn::Locale.eq(&loc))
                    .one(&txn)
                    .await?;

                if let Some(existing) = existing_trans {
                    let mut trans_active: BrandTranslationActiveModel = existing.into();
                    trans_active.name = Set(t.name);
                    trans_active.description = Set(t.description);
                    trans_active.updated_at = Set(now.into());
                    trans_active.update(&txn).await?;
                } else {
                    BrandTranslationActiveModel {
                        id: Set(Uuid::new_v4()),
                        brand_id: Set(id),
                        locale: Set(loc),
                        name: Set(t.name),
                        description: Set(t.description),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(&txn)
                    .await?;
                }
            }
        }

        txn.commit().await?;

        let all_translations = BrandTranslation::find()
            .filter(BrandTranslationColumn::BrandId.eq(id))
            .all(&self.db)
            .await?;

        Ok(Self::assemble_brand_dto(updated_brand, all_translations, None))
    }

    async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()> {
        let brand = Brand::find_by_id(id)
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BrandError::NotFound(id))?;

        Brand::delete_by_id(brand.id).exec(&self.db).await?;
        Ok(())
    }

    async fn get_brand_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BrandResult<Option<BrandDto>> {
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

    async fn assign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
        is_primary: bool,
    ) -> BrandResult<()> {
        // Ensure brand exists for tenant
        let _ = self.get_brand(tenant_id, brand_id, None).await?;

        let existing = BrandProduct::find()
            .filter(BrandProductColumn::TenantId.eq(tenant_id))
            .filter(BrandProductColumn::BrandId.eq(brand_id))
            .filter(BrandProductColumn::ProductId.eq(product_id))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            let mut active: BrandProductActiveModel = existing.into();
            active.is_primary = Set(is_primary);
            active.update(&self.db).await?;
        } else {
            BrandProductActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                brand_id: Set(brand_id),
                product_id: Set(product_id),
                is_primary: Set(is_primary),
                created_at: Set(Utc::now().into()),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    async fn unassign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
    ) -> BrandResult<()> {
        let link = BrandProduct::find()
            .filter(BrandProductColumn::TenantId.eq(tenant_id))
            .filter(BrandProductColumn::BrandId.eq(brand_id))
            .filter(BrandProductColumn::ProductId.eq(product_id))
            .one(&self.db)
            .await?;

        if let Some(link) = link {
            BrandProduct::delete_by_id(link.id).exec(&self.db).await?;
        }

        Ok(())
    }
}
