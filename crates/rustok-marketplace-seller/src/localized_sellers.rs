use std::collections::HashMap;

use chrono::Utc;
use rustok_api::normalize_locale_tag;
use rustok_core::generate_id;
use sea_orm::sea_query::{Alias, OnConflict, Query};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::dto::{
    MarketplaceSellerOnboardingStatus, MarketplaceSellerResponse, MarketplaceSellerStatus,
};
use crate::entities::{seller, seller_translation};
use crate::error::{MarketplaceSellerError, MarketplaceSellerResult};
use crate::seller_prose::{
    load_seller_prose, load_seller_prose_map, SellerProseProjection,
};

pub(crate) const MISSING_TRANSLATION_PREFIX: &str = "marketplace seller translation missing";

pub(crate) async fn load_seller_response<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let model = crate::service::find_seller(connection, tenant_id, seller_id).await?;
    let translation = resolve_translation(connection, tenant_id, seller_id, locale).await?;
    let prose = load_seller_prose(connection, tenant_id, seller_id).await?;
    map_seller(model, translation, prose)
}

pub(crate) async fn load_seller_responses<C: ConnectionTrait>(
    connection: &C,
    models: Vec<seller::Model>,
    locale: &str,
) -> MarketplaceSellerResult<Vec<MarketplaceSellerResponse>> {
    if models.is_empty() {
        return Ok(Vec::new());
    }

    let locale = normalize_seller_locale(locale)?;
    let tenant_id = models[0].tenant_id;
    let seller_ids = models.iter().map(|model| model.id).collect::<Vec<_>>();
    let translations = seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.is_in(seller_ids.clone()))
        .filter(seller_translation::Column::Locale.eq(locale.as_str()))
        .all(connection)
        .await?
        .into_iter()
        .map(|translation| (translation.seller_id, translation))
        .collect::<HashMap<_, _>>();
    let mut prose = load_seller_prose_map(connection, tenant_id, seller_ids).await?;

    models
        .into_iter()
        .map(|model| {
            let translation = translations
                .get(&model.id)
                .cloned()
                .ok_or_else(|| missing_translation_error(model.id, locale.as_str()))?;
            let projection = prose.remove(&model.id).unwrap_or_default();
            map_seller(model, translation, projection)
        })
        .collect()
}

pub(crate) async fn localized_seller_ids_for_search<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    locale: &str,
    search: &str,
) -> MarketplaceSellerResult<Vec<Uuid>> {
    let locale = normalize_seller_locale(locale)?;
    Ok(seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::Locale.eq(locale))
        .filter(seller_translation::Column::DisplayName.contains(search))
        .all(connection)
        .await?
        .into_iter()
        .map(|translation| translation.seller_id)
        .collect())
}

async fn resolve_translation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
) -> MarketplaceSellerResult<seller_translation::Model> {
    let locale = normalize_seller_locale(locale)?;
    seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .filter(seller_translation::Column::Locale.eq(locale.as_str()))
        .one(connection)
        .await?
        .ok_or_else(|| missing_translation_error(seller_id, locale.as_str()))
}

fn missing_translation_error(seller_id: Uuid, locale: &str) -> MarketplaceSellerError {
    MarketplaceSellerError::Validation(format!(
        "{MISSING_TRANSLATION_PREFIX}: seller {seller_id}, locale `{locale}`"
    ))
}

pub(crate) async fn upsert_translation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    seller_id: Uuid,
    locale: &str,
    display_name: String,
) -> MarketplaceSellerResult<seller_translation::Model> {
    let locale = normalize_seller_locale(locale)?;
    let now = Utc::now().fixed_offset();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("marketplace_seller_translations"))
        .columns([
            Alias::new("id"),
            Alias::new("tenant_id"),
            Alias::new("seller_id"),
            Alias::new("locale"),
            Alias::new("display_name"),
            Alias::new("created_at"),
            Alias::new("updated_at"),
        ])
        .values_panic([
            generate_id().into(),
            tenant_id.into(),
            seller_id.into(),
            locale.clone().into(),
            display_name.into(),
            now.into(),
            now.into(),
        ])
        .on_conflict(
            OnConflict::columns([
                Alias::new("tenant_id"),
                Alias::new("seller_id"),
                Alias::new("locale"),
            ])
            .update_column(Alias::new("display_name"))
            .update_column(Alias::new("updated_at"))
            .to_owned(),
        );
    connection
        .execute(connection.get_database_backend().build(&insert))
        .await?;

    seller_translation::Entity::find()
        .filter(seller_translation::Column::TenantId.eq(tenant_id))
        .filter(seller_translation::Column::SellerId.eq(seller_id))
        .filter(seller_translation::Column::Locale.eq(locale.as_str()))
        .one(connection)
        .await?
        .ok_or_else(|| missing_translation_error(seller_id, locale.as_str()))
}

pub(crate) fn normalize_seller_locale(value: &str) -> MarketplaceSellerResult<String> {
    normalize_locale_tag(value).ok_or_else(|| {
        MarketplaceSellerError::Validation(
            "locale must be a normalized BCP47-like tag with at most 32 bytes".to_string(),
        )
    })
}

fn map_seller(
    model: seller::Model,
    translation: seller_translation::Model,
    prose: SellerProseProjection,
) -> MarketplaceSellerResult<MarketplaceSellerResponse> {
    let status = MarketplaceSellerStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceSellerError::Validation(format!(
            "unknown marketplace seller status `{}`",
            model.status
        ))
    })?;
    let onboarding_status =
        MarketplaceSellerOnboardingStatus::parse(model.onboarding_status.as_str()).ok_or_else(
            || {
                MarketplaceSellerError::Validation(format!(
                    "unknown marketplace seller onboarding status `{}`",
                    model.onboarding_status
                ))
            },
        )?;
    let row_updated_at = model.updated_at;
    let onboarding_note = if prose
        .onboarding_at
        .is_some_and(|event_at| event_at >= row_updated_at)
    {
        prose.onboarding_note
    } else {
        model.onboarding_note
    };
    let suspension_reason = if prose
        .suspension_at
        .is_some_and(|event_at| event_at >= row_updated_at)
    {
        prose.suspension_reason
    } else {
        model.suspension_reason
    };

    Ok(MarketplaceSellerResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        handle: model.handle,
        resolved_locale: translation.locale,
        display_name: translation.display_name,
        legal_name: model.legal_name,
        status,
        onboarding_status,
        onboarding_note,
        suspension_reason,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
        activated_at: model.activated_at,
        suspended_at: model.suspended_at,
    })
}
