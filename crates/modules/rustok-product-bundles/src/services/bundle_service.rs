use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::dto::{
    BundleDto, BundleFilter, BundleItemDto, BundleItemInput, BundleListResponse,
    BundleTranslationDto, CreateBundleInput, UpdateBundleInput,
};
use crate::entities::{
    bundle::{ActiveModel as BundleActiveModel, Column as BundleColumn, Entity as Bundle},
    bundle_item::{
        ActiveModel as BundleItemActiveModel, Column as BundleItemColumn, Entity as BundleItem,
    },
    bundle_translation::{
        ActiveModel as BundleTranslationActiveModel, Column as BundleTranslationColumn,
        Entity as BundleTranslation,
    },
};
use crate::error::{BundleError, BundleResult};
use crate::ports::BundlePort;

#[derive(Clone)]
pub struct BundleService {
    db: DatabaseConnection,
}

impl BundleService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn assemble_bundle_dto(
        model: crate::entities::bundle::Model,
        translations: Vec<crate::entities::bundle_translation::Model>,
        items: Vec<crate::entities::bundle_item::Model>,
        locale: Option<&str>,
    ) -> BundleDto {
        let translation_dtos: Vec<BundleTranslationDto> = translations
            .iter()
            .map(|t| BundleTranslationDto {
                id: t.id,
                locale: t.locale.clone(),
                name: t.name.clone(),
                description: t.description.clone(),
            })
            .collect();

        let item_dtos: Vec<BundleItemDto> = items
            .iter()
            .map(|i| BundleItemDto {
                id: i.id,
                bundle_id: i.bundle_id,
                product_id: i.product_id,
                variant_id: i.variant_id,
                quantity: i.quantity,
                is_optional: i.is_optional,
                discount_rate: i.discount_rate,
                position: i.position,
                created_at: i.created_at.into(),
            })
            .collect();

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

        BundleDto {
            id: model.id,
            tenant_id: model.tenant_id,
            bundle_product_id: model.bundle_product_id,
            slug: model.slug,
            name,
            description,
            bundle_type: model.bundle_type,
            status: model.status,
            discount_type: model.discount_type,
            discount_value: model.discount_value,
            metadata: model.metadata,
            translations: translation_dtos,
            items: item_dtos,
            created_at: model.created_at.into(),
            updated_at: model.updated_at.into(),
        }
    }
}

#[async_trait]
impl BundlePort for BundleService {
    async fn get_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto> {
        let bundle = Bundle::find_by_id(id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(id))?;

        let translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.eq(id))
            .all(&self.db)
            .await?;

        let items = BundleItem::find()
            .filter(BundleItemColumn::BundleId.eq(id))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        Ok(Self::assemble_bundle_dto(
            bundle,
            translations,
            items,
            locale,
        ))
    }

    async fn get_bundle_by_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        locale: Option<&str>,
    ) -> BundleResult<BundleDto> {
        let bundle = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::Slug.eq(slug))
            .one(&self.db)
            .await?
            .ok_or_else(|| BundleError::SlugNotFound(slug.to_string()))?;

        let bundle_id = bundle.id;
        let translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.eq(bundle_id))
            .all(&self.db)
            .await?;

        let items = BundleItem::find()
            .filter(BundleItemColumn::BundleId.eq(bundle_id))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        Ok(Self::assemble_bundle_dto(
            bundle,
            translations,
            items,
            locale,
        ))
    }

    async fn list_bundles(
        &self,
        tenant_id: Uuid,
        filter: BundleFilter,
        page: u64,
        per_page: u64,
        locale: Option<&str>,
    ) -> BundleResult<BundleListResponse> {
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);

        let mut query = Bundle::find().filter(BundleColumn::TenantId.eq(tenant_id));

        if let Some(search) = &filter.search {
            let search_trim = search.trim();
            if !search_trim.is_empty() {
                query = query.filter(BundleColumn::Slug.contains(search_trim));
            }
        }

        if let Some(status) = &filter.status {
            query = query.filter(BundleColumn::Status.eq(status));
        }

        if let Some(btype) = &filter.bundle_type {
            query = query.filter(BundleColumn::BundleType.eq(btype));
        }

        let paginator = query
            .order_by_desc(BundleColumn::CreatedAt)
            .paginate(&self.db, per_page);

        let total = paginator.num_items().await?;
        let bundles = paginator.fetch_page(page - 1).await?;

        let bundle_ids: Vec<Uuid> = bundles.iter().map(|b| b.id).collect();

        let all_translations = if !bundle_ids.is_empty() {
            BundleTranslation::find()
                .filter(BundleTranslationColumn::BundleId.is_in(bundle_ids.clone()))
                .all(&self.db)
                .await?
        } else {
            vec![]
        };

        let all_items = if !bundle_ids.is_empty() {
            BundleItem::find()
                .filter(BundleItemColumn::BundleId.is_in(bundle_ids))
                .order_by_asc(BundleItemColumn::Position)
                .all(&self.db)
                .await?
        } else {
            vec![]
        };

        let dtos = bundles
            .into_iter()
            .map(|b| {
                let trans = all_translations
                    .iter()
                    .filter(|t| t.bundle_id == b.id)
                    .cloned()
                    .collect();
                let items = all_items
                    .iter()
                    .filter(|i| i.bundle_id == b.id)
                    .cloned()
                    .collect();
                Self::assemble_bundle_dto(b, trans, items, locale)
            })
            .collect();

        Ok(BundleListResponse {
            items: dtos,
            total,
            page,
            per_page,
        })
    }

    async fn create_bundle(
        &self,
        tenant_id: Uuid,
        input: CreateBundleInput,
    ) -> BundleResult<BundleDto> {
        let slug = input.slug.trim().to_lowercase();
        if slug.is_empty() {
            return Err(BundleError::InvalidInput("Slug cannot be empty".into()));
        }

        let existing = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::Slug.eq(&slug))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(BundleError::SlugAlreadyExists(slug));
        }

        let txn = self.db.begin().await?;

        let bundle_id = Uuid::new_v4();
        let now = Utc::now();

        let bundle_active = BundleActiveModel {
            id: Set(bundle_id),
            tenant_id: Set(tenant_id),
            bundle_product_id: Set(input.bundle_product_id),
            slug: Set(slug),
            bundle_type: Set(input.bundle_type.unwrap_or_else(|| "fixed".into())),
            status: Set(input.status.unwrap_or_else(|| "active".into())),
            discount_type: Set(input.discount_type.unwrap_or_else(|| "none".into())),
            discount_value: Set(input.discount_value.unwrap_or_default()),
            metadata: Set(input.metadata.unwrap_or_else(|| serde_json::json!({}))),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let inserted_bundle = bundle_active.insert(&txn).await?;

        let mut inserted_translations = Vec::new();
        for trans_input in input.translations {
            let trans_active = BundleTranslationActiveModel {
                id: Set(Uuid::new_v4()),
                bundle_id: Set(bundle_id),
                locale: Set(trans_input.locale),
                name: Set(trans_input.name),
                description: Set(trans_input.description),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            };
            inserted_translations.push(trans_active.insert(&txn).await?);
        }

        let mut inserted_items = Vec::new();
        for (idx, item_input) in input.items.into_iter().enumerate() {
            if item_input.quantity < 1 {
                return Err(BundleError::InvalidInput(
                    "Item quantity must be at least 1".into(),
                ));
            }
            let item_active = BundleItemActiveModel {
                id: Set(Uuid::new_v4()),
                bundle_id: Set(bundle_id),
                product_id: Set(item_input.product_id),
                variant_id: Set(item_input.variant_id),
                quantity: Set(item_input.quantity),
                is_optional: Set(item_input.is_optional.unwrap_or(false)),
                discount_rate: Set(item_input.discount_rate),
                position: Set(item_input.position.unwrap_or(idx as i32)),
                created_at: Set(now.into()),
            };
            inserted_items.push(item_active.insert(&txn).await?);
        }

        txn.commit().await?;

        Ok(Self::assemble_bundle_dto(
            inserted_bundle,
            inserted_translations,
            inserted_items,
            None,
        ))
    }

    async fn update_bundle(
        &self,
        tenant_id: Uuid,
        id: Uuid,
        input: UpdateBundleInput,
    ) -> BundleResult<BundleDto> {
        let bundle = Bundle::find_by_id(id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(id))?;

        let txn = self.db.begin().await?;
        let now = Utc::now();
        let mut active: BundleActiveModel = bundle.into();

        if let Some(slug) = input.slug {
            let slug_clean = slug.trim().to_lowercase();
            if slug_clean.is_empty() {
                return Err(BundleError::InvalidInput("Slug cannot be empty".into()));
            }
            let existing = Bundle::find()
                .filter(BundleColumn::TenantId.eq(tenant_id))
                .filter(BundleColumn::Slug.eq(&slug_clean))
                .filter(BundleColumn::Id.ne(id))
                .one(&txn)
                .await?;

            if existing.is_some() {
                return Err(BundleError::SlugAlreadyExists(slug_clean));
            }
            active.slug = Set(slug_clean);
        }

        if let Some(bundle_product_id) = input.bundle_product_id {
            active.bundle_product_id = Set(bundle_product_id);
        }

        if let Some(btype) = input.bundle_type {
            active.bundle_type = Set(btype);
        }

        if let Some(status) = input.status {
            active.status = Set(status);
        }

        if let Some(dtype) = input.discount_type {
            active.discount_type = Set(dtype);
        }

        if let Some(dval) = input.discount_value {
            active.discount_value = Set(dval);
        }

        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }

        active.updated_at = Set(now.into());
        let updated_bundle = active.update(&txn).await?;

        if let Some(translations) = input.translations {
            for trans_in in translations {
                let existing_trans = BundleTranslation::find()
                    .filter(BundleTranslationColumn::BundleId.eq(id))
                    .filter(BundleTranslationColumn::Locale.eq(&trans_in.locale))
                    .one(&txn)
                    .await?;

                if let Some(existing) = existing_trans {
                    let mut trans_act: BundleTranslationActiveModel = existing.into();
                    trans_act.name = Set(trans_in.name);
                    trans_act.description = Set(trans_in.description);
                    trans_act.updated_at = Set(now.into());
                    trans_act.update(&txn).await?;
                } else {
                    let new_trans = BundleTranslationActiveModel {
                        id: Set(Uuid::new_v4()),
                        bundle_id: Set(id),
                        locale: Set(trans_in.locale),
                        name: Set(trans_in.name),
                        description: Set(trans_in.description),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                    };
                    new_trans.insert(&txn).await?;
                }
            }
        }

        txn.commit().await?;

        let translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.eq(id))
            .all(&self.db)
            .await?;

        let items = BundleItem::find()
            .filter(BundleItemColumn::BundleId.eq(id))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        Ok(Self::assemble_bundle_dto(
            updated_bundle,
            translations,
            items,
            None,
        ))
    }

    async fn delete_bundle(&self, tenant_id: Uuid, id: Uuid) -> BundleResult<()> {
        let bundle = Bundle::find_by_id(id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(id))?;

        let txn = self.db.begin().await?;
        BundleTranslation::delete_many()
            .filter(BundleTranslationColumn::BundleId.eq(id))
            .exec(&txn)
            .await?;

        BundleItem::delete_many()
            .filter(BundleItemColumn::BundleId.eq(id))
            .exec(&txn)
            .await?;

        Bundle::delete_by_id(bundle.id).exec(&txn).await?;
        txn.commit().await?;

        Ok(())
    }

    async fn add_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item: BundleItemInput,
    ) -> BundleResult<BundleItemDto> {
        let _ = Bundle::find_by_id(bundle_id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(bundle_id))?;

        if item.quantity < 1 {
            return Err(BundleError::InvalidInput(
                "Item quantity must be at least 1".into(),
            ));
        }

        let now = Utc::now();
        let item_id = Uuid::new_v4();

        let position = match item.position {
            Some(p) => p,
            None => {
                let count = BundleItem::find()
                    .filter(BundleItemColumn::BundleId.eq(bundle_id))
                    .count(&self.db)
                    .await?;
                count as i32
            }
        };

        let active = BundleItemActiveModel {
            id: Set(item_id),
            bundle_id: Set(bundle_id),
            product_id: Set(item.product_id),
            variant_id: Set(item.variant_id),
            quantity: Set(item.quantity),
            is_optional: Set(item.is_optional.unwrap_or(false)),
            discount_rate: Set(item.discount_rate),
            position: Set(position),
            created_at: Set(now.into()),
        };

        let inserted = active.insert(&self.db).await?;

        Ok(BundleItemDto {
            id: inserted.id,
            bundle_id: inserted.bundle_id,
            product_id: inserted.product_id,
            variant_id: inserted.variant_id,
            quantity: inserted.quantity,
            is_optional: inserted.is_optional,
            discount_rate: inserted.discount_rate,
            position: inserted.position,
            created_at: inserted.created_at.into(),
        })
    }

    async fn remove_bundle_item(
        &self,
        tenant_id: Uuid,
        bundle_id: Uuid,
        item_id: Uuid,
    ) -> BundleResult<()> {
        let _ = Bundle::find_by_id(bundle_id)
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::NotFound(bundle_id))?;

        let item = BundleItem::find_by_id(item_id)
            .filter(BundleItemColumn::BundleId.eq(bundle_id))
            .one(&self.db)
            .await?
            .ok_or(BundleError::ItemNotFound(item_id))?;

        BundleItem::delete_by_id(item.id).exec(&self.db).await?;

        Ok(())
    }

    async fn get_bundles_for_product(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: Option<&str>,
    ) -> BundleResult<Vec<BundleDto>> {
        let bundle_items = BundleItem::find()
            .filter(BundleItemColumn::ProductId.eq(product_id))
            .all(&self.db)
            .await?;

        let bundle_ids: Vec<Uuid> = bundle_items.into_iter().map(|bi| bi.bundle_id).collect();
        if bundle_ids.is_empty() {
            return Ok(vec![]);
        }

        let bundles = Bundle::find()
            .filter(BundleColumn::TenantId.eq(tenant_id))
            .filter(BundleColumn::Id.is_in(bundle_ids))
            .all(&self.db)
            .await?;

        let ids: Vec<Uuid> = bundles.iter().map(|b| b.id).collect();

        let all_translations = BundleTranslation::find()
            .filter(BundleTranslationColumn::BundleId.is_in(ids.clone()))
            .all(&self.db)
            .await?;

        let all_items = BundleItem::find()
            .filter(BundleItemColumn::BundleId.is_in(ids))
            .order_by_asc(BundleItemColumn::Position)
            .all(&self.db)
            .await?;

        let dtos = bundles
            .into_iter()
            .map(|b| {
                let trans = all_translations
                    .iter()
                    .filter(|t| t.bundle_id == b.id)
                    .cloned()
                    .collect();
                let items = all_items
                    .iter()
                    .filter(|i| i.bundle_id == b.id)
                    .cloned()
                    .collect();
                Self::assemble_bundle_dto(b, trans, items, locale)
            })
            .collect();

        Ok(dtos)
    }
}
