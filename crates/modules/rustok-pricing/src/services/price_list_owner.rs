use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rustok_api::TenantLocale;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::generate_id;
use rustok_pricing_persistence::entities::{price_list, price_list_translation};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::translation_changes::{
    PriceListTranslationChangeLifecycle, record_price_list_translation_change_in_tx,
};

use super::translation::{
    PriceListTranslationExactLocaleError, resource_revision as price_list_copy_revision,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceListOwnerTranslationInput {
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePriceListOwnerInput {
    pub translations: Vec<PriceListOwnerTranslationInput>,
    pub list_type: String,
    pub status: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdatePriceListOwnerInput {
    pub translations: Option<Vec<PriceListOwnerTranslationInput>>,
    pub list_type: Option<String>,
    pub status: Option<String>,
    pub starts_at: Option<Option<DateTime<Utc>>>,
    pub ends_at: Option<Option<DateTime<Utc>>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceListOwnerSnapshot {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub list_type: String,
    pub status: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<rust_decimal::Decimal>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub translations: Vec<PriceListOwnerTranslationInput>,
}

/// Canonical Pricing-owned lifecycle boundary for Price List identity and localized copy.
///
/// Translation workflows mutate exact locales through `PriceListTranslationService`, while native
/// Pricing creation/update/delete must use this service so the parent row, localized copy, and
/// durable Translation change journal share one transaction. Operational rule/scope/price writes
/// remain separate Pricing concerns and deliberately do not change the localized-copy revision.
pub struct PriceListOwnerService {
    db: DatabaseConnection,
}

impl PriceListOwnerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_price_list(
        &self,
        tenant_id: Uuid,
        input: CreatePriceListOwnerInput,
    ) -> CommerceResult<PriceListOwnerSnapshot> {
        validate_tenant(tenant_id)?;
        let translations = normalize_translations(input.translations)?;
        let list_type = normalize_short_token("list_type", &input.list_type)?;
        let status = normalize_short_token("status", &input.status)?;
        validate_window(input.starts_at.as_ref(), input.ends_at.as_ref())?;
        let channel_slug = normalize_channel_slug(input.channel_slug.as_deref());

        let txn = self.db.begin().await?;
        let now = Utc::now();
        let model = price_list::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            r#type: Set(list_type),
            status: Set(status),
            channel_id: Set(input.channel_id),
            channel_slug: Set(channel_slug),
            rule_kind: Set(None),
            adjustment_percent: Set(None),
            starts_at: Set(input.starts_at.map(Into::into)),
            ends_at: Set(input.ends_at.map(Into::into)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;
        insert_translations(&txn, model.id, &translations).await?;
        let persisted = load_translations(&txn, model.id).await?;
        let revision = price_list_copy_revision(&model, &persisted);
        record_price_list_translation_change_in_tx(
            &txn,
            tenant_id,
            model.id,
            generate_id(),
            &revision,
            PriceListTranslationChangeLifecycle::Active,
        )
        .await
        .map_err(translation_change_error_to_commerce_error)?;
        txn.commit().await?;

        Ok(snapshot(model, persisted))
    }

    pub async fn update_price_list(
        &self,
        tenant_id: Uuid,
        price_list_id: Uuid,
        input: UpdatePriceListOwnerInput,
    ) -> CommerceResult<PriceListOwnerSnapshot> {
        validate_tenant(tenant_id)?;
        validate_price_list_id(price_list_id)?;

        let normalized_translations = input
            .translations
            .map(normalize_translations)
            .transpose()?;
        let list_type = input
            .list_type
            .as_deref()
            .map(|value| normalize_short_token("list_type", value))
            .transpose()?;
        let status = input
            .status
            .as_deref()
            .map(|value| normalize_short_token("status", value))
            .transpose()?;

        let txn = self.db.begin().await?;
        let current = load_locked_price_list(&txn, tenant_id, price_list_id).await?;
        let existing_translations = load_translations(&txn, price_list_id).await?;

        let next_starts_at = match &input.starts_at {
            Some(value) => value.clone(),
            None => current
                .starts_at
                .as_ref()
                .map(|value| value.with_timezone(&Utc)),
        };
        let next_ends_at = match &input.ends_at {
            Some(value) => value.clone(),
            None => current.ends_at.as_ref().map(|value| value.with_timezone(&Utc)),
        };
        validate_window(next_starts_at.as_ref(), next_ends_at.as_ref())?;

        let mut active: price_list::ActiveModel = current.into();
        if let Some(list_type) = list_type {
            active.r#type = Set(list_type);
        }
        if let Some(status) = status {
            active.status = Set(status);
        }
        if let Some(starts_at) = input.starts_at {
            active.starts_at = Set(starts_at.map(Into::into));
        }
        if let Some(ends_at) = input.ends_at {
            active.ends_at = Set(ends_at.map(Into::into));
        }
        active.updated_at = Set(Utc::now().into());
        let model = active.update(&txn).await?;

        let copy_changed = normalized_translations
            .as_ref()
            .is_some_and(|next| !translations_semantically_equal(&existing_translations, next));
        let persisted = match normalized_translations {
            Some(translations) if copy_changed => {
                price_list_translation::Entity::delete_many()
                    .filter(price_list_translation::Column::PriceListId.eq(price_list_id))
                    .exec(&txn)
                    .await?;
                insert_translations(&txn, price_list_id, &translations).await?;
                load_translations(&txn, price_list_id).await?
            }
            _ => existing_translations,
        };

        if copy_changed {
            let revision = price_list_copy_revision(&model, &persisted);
            record_price_list_translation_change_in_tx(
                &txn,
                tenant_id,
                price_list_id,
                generate_id(),
                &revision,
                PriceListTranslationChangeLifecycle::Active,
            )
            .await
            .map_err(translation_change_error_to_commerce_error)?;
        }
        txn.commit().await?;

        Ok(snapshot(model, persisted))
    }

    pub async fn delete_price_list(
        &self,
        tenant_id: Uuid,
        price_list_id: Uuid,
    ) -> CommerceResult<()> {
        validate_tenant(tenant_id)?;
        validate_price_list_id(price_list_id)?;

        let txn = self.db.begin().await?;
        let model = load_locked_price_list(&txn, tenant_id, price_list_id).await?;
        let translations = load_translations(&txn, price_list_id).await?;
        let revision = price_list_copy_revision(&model, &translations);
        record_price_list_translation_change_in_tx(
            &txn,
            tenant_id,
            price_list_id,
            generate_id(),
            &revision,
            PriceListTranslationChangeLifecycle::Deleted,
        )
        .await
        .map_err(translation_change_error_to_commerce_error)?;
        price_list::Entity::delete_by_id(price_list_id).exec(&txn).await?;
        txn.commit().await?;
        Ok(())
    }
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn validate_price_list_id(price_list_id: Uuid) -> CommerceResult<()> {
    if price_list_id.is_nil() {
        return Err(CommerceError::Validation(
            "price_list_id must not be nil".to_string(),
        ));
    }
    Ok(())
}

fn normalize_short_token(field: &str, value: &str) -> CommerceResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value.chars().count() > 32 {
        return Err(CommerceError::Validation(format!(
            "{field} must contain between 1 and 32 characters"
        )));
    }
    Ok(value)
}

fn normalize_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_translations(
    translations: Vec<PriceListOwnerTranslationInput>,
) -> CommerceResult<Vec<PriceListOwnerTranslationInput>> {
    if translations.is_empty() {
        return Err(CommerceError::Validation(
            "at least one price-list translation is required".to_string(),
        ));
    }

    let mut normalized = BTreeMap::new();
    for translation in translations {
        let locale = TenantLocale::new(&translation.locale)
            .map(TenantLocale::into_inner)
            .map_err(|error| CommerceError::Validation(error.to_string()))?;
        let name = translation.name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(CommerceError::Validation(
                "price-list translation name must contain between 1 and 100 characters"
                    .to_string(),
            ));
        }
        let description = translation
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if normalized
            .insert(
                locale.clone(),
                PriceListOwnerTranslationInput {
                    locale,
                    name: name.to_string(),
                    description,
                },
            )
            .is_some()
        {
            return Err(CommerceError::Validation(
                "price-list translations must contain unique locales".to_string(),
            ));
        }
    }
    Ok(normalized.into_values().collect())
}

fn validate_window(
    starts_at: Option<&DateTime<Utc>>,
    ends_at: Option<&DateTime<Utc>>,
) -> CommerceResult<()> {
    if let (Some(starts_at), Some(ends_at)) = (starts_at, ends_at)
        && starts_at > ends_at
    {
        return Err(CommerceError::Validation(
            "price-list starts_at must not be after ends_at".to_string(),
        ));
    }
    Ok(())
}

async fn load_locked_price_list<C>(
    db: &C,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<price_list::Model>
where
    C: sea_orm::ConnectionTrait,
{
    price_list::Entity::find_by_id(price_list_id)
        .filter(price_list::Column::TenantId.eq(tenant_id))
        .lock_exclusive()
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))
}

async fn insert_translations<C>(
    db: &C,
    price_list_id: Uuid,
    translations: &[PriceListOwnerTranslationInput],
) -> CommerceResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    for translation in translations {
        price_list_translation::ActiveModel {
            id: Set(generate_id()),
            price_list_id: Set(price_list_id),
            locale: Set(translation.locale.clone()),
            name: Set(translation.name.clone()),
            description: Set(translation.description.clone()),
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn load_translations<C>(
    db: &C,
    price_list_id: Uuid,
) -> CommerceResult<Vec<price_list_translation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(price_list_translation::Entity::find()
        .filter(price_list_translation::Column::PriceListId.eq(price_list_id))
        .order_by_asc(price_list_translation::Column::Locale)
        .all(db)
        .await?)
}

fn translations_semantically_equal(
    current: &[price_list_translation::Model],
    next: &[PriceListOwnerTranslationInput],
) -> bool {
    current.len() == next.len()
        && current.iter().zip(next).all(|(current, next)| {
            current.locale == next.locale
                && current.name == next.name
                && current.description == next.description
        })
}

fn snapshot(
    model: price_list::Model,
    translations: Vec<price_list_translation::Model>,
) -> PriceListOwnerSnapshot {
    PriceListOwnerSnapshot {
        id: model.id,
        tenant_id: model.tenant_id,
        list_type: model.r#type,
        status: model.status,
        channel_id: model.channel_id,
        channel_slug: model.channel_slug,
        rule_kind: model.rule_kind,
        adjustment_percent: model.adjustment_percent,
        starts_at: model
            .starts_at
            .map(|value| value.with_timezone(&Utc)),
        ends_at: model.ends_at.map(|value| value.with_timezone(&Utc)),
        translations: translations
            .into_iter()
            .map(|translation| PriceListOwnerTranslationInput {
                locale: translation.locale,
                name: translation.name,
                description: translation.description,
            })
            .collect(),
    }
}

fn translation_change_error_to_commerce_error(
    error: PriceListTranslationExactLocaleError,
) -> CommerceError {
    match error {
        PriceListTranslationExactLocaleError::Database(error) => CommerceError::Database(error),
        other => CommerceError::Validation(format!(
            "Pricing translation change journal write failed: {other}"
        )),
    }
}
