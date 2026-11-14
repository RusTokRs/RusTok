use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use uuid::Uuid;

use crate::{
    dto::{BrandDto, CreateBrandInput, UpdateBrandInput},
    entities::{
        brand::{ActiveModel as BrandActiveModel, Column as BrandColumn, Entity as Brand},
        brand_category::{
            ActiveModel as BrandCategoryActiveModel, Column as BrandCategoryColumn,
            Entity as BrandCategory,
        },
        brand_product::{
            ActiveModel as BrandProductActiveModel, Column as BrandProductColumn,
            Entity as BrandProduct,
        },
        brand_translation::{
            ActiveModel as BrandTranslationActiveModel, Column as BrandTranslationColumn,
            Entity as BrandTranslation,
        },
    },
    error::{BrandError, BrandResult},
};

use super::{
    BrandService,
    repository::{assemble_brand_dto, load_translations},
};

impl BrandService {
    pub(super) async fn create_brand(
        &self,
        tenant_id: Uuid,
        input: CreateBrandInput,
    ) -> BrandResult<BrandDto> {
        if tenant_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Tenant ID cannot be nil".to_string(),
            ));
        }
        let slug = input.slug.trim().to_lowercase();
        if slug.is_empty() {
            return Err(BrandError::InvalidInput("Slug cannot be empty".to_string()));
        }
        if slug.len() > 100 {
            return Err(BrandError::InvalidInput(
                "Slug exceeds maximum length of 100 characters".to_string(),
            ));
        }
        if input
            .website_url
            .as_ref()
            .is_some_and(|url| url.len() > 512)
        {
            return Err(BrandError::InvalidInput(
                "Website URL exceeds maximum length of 512 characters".to_string(),
            ));
        }
        for t in &input.translations {
            if t.locale.trim().is_empty() {
                return Err(BrandError::InvalidInput(
                    "Translation locale cannot be empty".to_string(),
                ));
            }
            if t.locale.len() > 16 {
                return Err(BrandError::InvalidInput(
                    "Translation locale exceeds maximum length of 16 characters".to_string(),
                ));
            }
            if t.name.trim().is_empty() {
                return Err(BrandError::InvalidInput(
                    "Translation name cannot be empty".to_string(),
                ));
            }
            if t.name.len() > 255 {
                return Err(BrandError::InvalidInput(
                    "Translation name exceeds maximum length of 255 characters".to_string(),
                ));
            }
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
                locale: Set(t.locale.trim().to_lowercase()),
                name: Set(t.name.trim().to_string()),
                description: Set(t.description),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
            trans_models.push(trans_model);
        }

        txn.commit().await?;

        Ok(assemble_brand_dto(brand_model, trans_models, None))
    }

    pub(super) async fn update_brand(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBrandInput,
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

        let txn = self.db.begin().await?;

        let mut active: BrandActiveModel = brand.into();
        let now = Utc::now();
        active.updated_at = Set(now.into());

        if let Some(slug) = input.slug {
            let clean_slug = slug.trim().to_lowercase();
            if clean_slug.is_empty() {
                return Err(BrandError::InvalidInput("Slug cannot be empty".to_string()));
            }
            if clean_slug.len() > 100 {
                return Err(BrandError::InvalidInput(
                    "Slug exceeds maximum length of 100 characters".to_string(),
                ));
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
            if url.as_ref().is_some_and(|u| u.len() > 512) {
                return Err(BrandError::InvalidInput(
                    "Website URL exceeds maximum length of 512 characters".to_string(),
                ));
            }
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
                let loc = t.locale.trim().to_lowercase();
                if loc.is_empty() {
                    return Err(BrandError::InvalidInput(
                        "Translation locale cannot be empty".to_string(),
                    ));
                }
                if loc.len() > 16 {
                    return Err(BrandError::InvalidInput(
                        "Translation locale exceeds maximum length of 16 characters".to_string(),
                    ));
                }
                let name = t.name.trim().to_string();
                if name.is_empty() {
                    return Err(BrandError::InvalidInput(
                        "Translation name cannot be empty".to_string(),
                    ));
                }
                if name.len() > 255 {
                    return Err(BrandError::InvalidInput(
                        "Translation name exceeds maximum length of 255 characters".to_string(),
                    ));
                }

                let existing_trans = BrandTranslation::find()
                    .filter(BrandTranslationColumn::BrandId.eq(id))
                    .filter(BrandTranslationColumn::Locale.eq(&loc))
                    .one(&txn)
                    .await?;

                if let Some(existing) = existing_trans {
                    let mut trans_active: BrandTranslationActiveModel = existing.into();
                    trans_active.name = Set(name);
                    trans_active.description = Set(t.description);
                    trans_active.updated_at = Set(now.into());
                    trans_active.update(&txn).await?;
                } else {
                    BrandTranslationActiveModel {
                        id: Set(Uuid::new_v4()),
                        brand_id: Set(id),
                        locale: Set(loc),
                        name: Set(name),
                        description: Set(t.description),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    }
                    .insert(&txn)
                    .await?;
                }
            }
        }

        // Read back the result before commit so a snapshot error rolls back
        // the update instead of reporting a failure after the write succeeded.
        let all_translations = load_translations(&txn, id).await?;
        txn.commit().await?;

        Ok(assemble_brand_dto(updated_brand, all_translations, None))
    }

    pub(super) async fn delete_brand(&self, tenant_id: Uuid, id: Uuid) -> BrandResult<()> {
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

        let txn = self.db.begin().await?;

        BrandCategory::delete_many()
            .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
            .filter(BrandCategoryColumn::BrandId.eq(brand.id))
            .exec(&txn)
            .await?;

        BrandProduct::delete_many()
            .filter(BrandProductColumn::TenantId.eq(tenant_id))
            .filter(BrandProductColumn::BrandId.eq(brand.id))
            .exec(&txn)
            .await?;

        BrandTranslation::delete_many()
            .filter(BrandTranslationColumn::BrandId.eq(brand.id))
            .exec(&txn)
            .await?;

        Brand::delete_many()
            .filter(BrandColumn::TenantId.eq(tenant_id))
            .filter(BrandColumn::Id.eq(brand.id))
            .exec(&txn)
            .await?;

        txn.commit().await?;
        Ok(())
    }

    pub(super) async fn assign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
        is_primary: bool,
    ) -> BrandResult<()> {
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
        if product_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Product ID cannot be nil".to_string(),
            ));
        }

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

    pub(super) async fn unassign_product_brand(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        product_id: Uuid,
    ) -> BrandResult<()> {
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
        if product_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Product ID cannot be nil".to_string(),
            ));
        }

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

    pub(super) async fn assign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
        is_featured: bool,
    ) -> BrandResult<()> {
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
        if category_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Category ID cannot be nil".to_string(),
            ));
        }

        // Ensure brand exists for tenant
        let _ = self.get_brand(tenant_id, brand_id, None).await?;

        let existing = BrandCategory::find()
            .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
            .filter(BrandCategoryColumn::BrandId.eq(brand_id))
            .filter(BrandCategoryColumn::CategoryId.eq(category_id))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            let mut active: BrandCategoryActiveModel = existing.into();
            active.is_featured = Set(is_featured);
            active.update(&self.db).await?;
        } else {
            BrandCategoryActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                brand_id: Set(brand_id),
                category_id: Set(category_id),
                is_featured: Set(is_featured),
                created_at: Set(Utc::now().into()),
            }
            .insert(&self.db)
            .await?;
        }

        Ok(())
    }

    pub(super) async fn unassign_brand_category(
        &self,
        tenant_id: Uuid,
        brand_id: Uuid,
        category_id: Uuid,
    ) -> BrandResult<()> {
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
        if category_id.is_nil() {
            return Err(BrandError::InvalidInput(
                "Category ID cannot be nil".to_string(),
            ));
        }

        let link = BrandCategory::find()
            .filter(BrandCategoryColumn::TenantId.eq(tenant_id))
            .filter(BrandCategoryColumn::BrandId.eq(brand_id))
            .filter(BrandCategoryColumn::CategoryId.eq(category_id))
            .one(&self.db)
            .await?;

        if let Some(link) = link {
            BrandCategory::delete_by_id(link.id).exec(&self.db).await?;
        }

        Ok(())
    }
}
