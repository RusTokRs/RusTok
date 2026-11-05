use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QuerySelect, Statement,
};
use uuid::Uuid;

use rustok_channel::entities::channel;
use rustok_commerce_foundation::entities::product_variant;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

use crate::entities;
use super::resolution::{self, ResolutionError};
use super::types::normalize_channel_slug;

pub(crate) async fn validate_channel_scope_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id must not be a nil UUID".to_string(),
        ));
    }
    if let Some(channel_id) = channel_id
        && channel_id.is_nil()
    {
        return Err(CommerceError::Validation(
            "channel_id must not be a nil UUID".to_string(),
        ));
    }
    let normalized_slug = normalize_channel_slug(channel_slug);
    match channel_id {
        Some(channel_id) => {
            let model = channel::Entity::find_by_id(channel_id)
                .filter(channel::Column::TenantId.eq(tenant_id))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    CommerceError::Validation(
                        "channel_id must reference a channel owned by the price-list tenant"
                            .to_string(),
                    )
                })?;
            if let Some(slug) = normalized_slug.as_deref()
                && slug
                    != normalize_channel_slug(Some(model.slug.as_str()))
                        .as_deref()
                        .unwrap_or_default()
            {
                return Err(CommerceError::Validation(
                    "channel_id and channel_slug must reference the same tenant channel"
                        .to_string(),
                ));
            }
        }
        None => {
            if let Some(slug) = normalized_slug {
                let exists = channel::Entity::find()
                    .filter(channel::Column::TenantId.eq(tenant_id))
                    .filter(channel::Column::Slug.eq(slug.clone()))
                    .one(txn)
                    .await?
                    .is_some();
                if !exists {
                    return Err(CommerceError::Validation(
                        "channel_slug must reference a channel owned by the price-list tenant"
                            .to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn load_variant_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<product_variant::Model> {
    if tenant_id.is_nil() || variant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and variant_id must not be nil UUIDs".to_string(),
        ));
    }
    let query = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id));
    let variant = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE product_variants SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), variant_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    variant.ok_or(CommerceError::VariantNotFound(variant_id))
}

pub(crate) async fn resolve_active_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model> {
    if tenant_id.is_nil() || price_list_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and price_list_id must not be nil UUIDs".to_string(),
        ));
    }
    let price_list = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

pub(crate) async fn resolve_active_price_list_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model> {
    if tenant_id.is_nil() || price_list_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and price_list_id must not be nil UUIDs".to_string(),
        ));
    }
    let query = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id));
    let price_list = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE price_lists SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), price_list_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    }
    .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_active_price_list(price_list: &entities::price_list::Model) -> CommerceResult<()> {
    validate_active_price_list_at(price_list, Utc::now())
        .map_err(ResolutionError::into_commerce)
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_active_price_list_at(
    price_list: &entities::price_list::Model,
    now: DateTime<Utc>,
) -> Result<(), ResolutionError> {
    if price_list.channel_id.is_some_and(|id| id.is_nil())
        || price_list
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
        || !matches!(price_list.r#type.as_str(), "sale" | "override")
        || price_list
            .starts_at
            .zip(price_list.ends_at)
            .is_some_and(|(start, end)| start >= end)
    {
        return Err(resolution::invalid_data(
            "Stored price-list scope, type or window is invalid",
        ));
    }
    if !price_list.status.eq_ignore_ascii_case("active") {
        return Err(CommerceError::Validation(
            "price_list_id must reference an active price list".to_string(),
        )
        .into());
    }

    if price_list
        .starts_at
        .as_ref()
        .is_some_and(|starts_at| *starts_at > now)
    {
        return Err(
            CommerceError::Validation("price_list_id is not active yet".to_string()).into(),
        );
    }
    if price_list
        .ends_at
        .as_ref()
        .is_some_and(|ends_at| *ends_at <= now)
    {
        return Err(
            CommerceError::Validation("price_list_id is already expired".to_string()).into(),
        );
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
pub(crate) fn validate_or_inherit_price_list_scope(
    price_list: &entities::price_list::Model,
    channel_id: Option<Uuid>,
    channel_slug: Option<String>,
) -> CommerceResult<(Option<Uuid>, Option<String>)> {
    let channel_slug = normalize_channel_slug(channel_slug.as_deref());

    if channel_id.is_none() && channel_slug.is_none() {
        return Ok((price_list.channel_id, price_list.channel_slug.clone()));
    }

    if channel_id != price_list.channel_id
        || channel_slug.as_deref()
            != normalize_channel_slug(price_list.channel_slug.as_deref()).as_deref()
    {
        return Err(CommerceError::Validation(
            "price rows for a selected price_list_id must match the price list channel scope"
                .to_string(),
        ));
    }

    Ok((channel_id, channel_slug))
}
