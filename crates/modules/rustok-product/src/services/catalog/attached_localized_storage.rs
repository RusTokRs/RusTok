use std::collections::BTreeMap;

use chrono::Utc;
use rustok_api::normalize_locale_tag;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Select, Set,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::entities::product_attached_localized_value::{ActiveModel, Column, Entity};
use crate::{CommerceError, CommerceResult};

fn product_rows(tenant_id: Uuid, product_id: Uuid) -> Select<Entity> {
    Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::ProductId.eq(product_id))
}

pub(super) async fn load_exact(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
) -> CommerceResult<Value> {
    let locale = normalize_locale_tag(locale)
        .ok_or_else(|| CommerceError::Validation("Custom field locale is invalid".into()))?;
    let rows = product_rows(tenant_id, product_id)
        .filter(Column::Locale.eq(locale))
        .all(db)
        .await?;
    Ok(Value::Object(
        rows.into_iter()
            .map(|row| (row.field_key, row.value))
            .collect(),
    ))
}

pub(super) async fn load_by_locale(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<BTreeMap<String, Map<String, Value>>> {
    let mut localized = BTreeMap::<String, Map<String, Value>>::new();
    for row in product_rows(tenant_id, product_id).all(db).await? {
        let locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
            CommerceError::Database(sea_orm::DbErr::Custom(
                "Invalid stored Product locale".into(),
            ))
        })?;
        if localized
            .entry(locale)
            .or_default()
            .insert(row.field_key, row.value)
            .is_some()
        {
            return Err(CommerceError::Database(sea_orm::DbErr::Custom(
                "Duplicate stored Product localized field".into(),
            )));
        }
    }
    Ok(localized)
}

/// Replace the exact-locale values inside the locked Product write transaction.
pub(super) async fn replace_for_locale(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
    locale: &str,
    values: &Value,
) -> CommerceResult<()> {
    let locale = normalize_locale_tag(locale)
        .ok_or_else(|| CommerceError::Validation("Custom field locale is invalid".into()))?;
    let values = values.as_object().ok_or_else(|| {
        CommerceError::Validation("Expected an object for localized Product fields".into())
    })?;
    let rows = product_rows(tenant_id, product_id)
        .filter(Column::Locale.eq(&locale))
        .all(db)
        .await?;
    let mut existing: BTreeMap<_, _> = rows
        .into_iter()
        .map(|row| (row.field_key.clone(), row))
        .collect();
    let now = Utc::now();
    for (key, value) in values {
        if let Some(row) = existing.remove(key) {
            if row.value != *value {
                let mut row: ActiveModel = row.into();
                row.value = Set(value.clone());
                row.updated_at = Set(now.into());
                row.update(db).await?;
            }
        } else {
            ActiveModel {
                id: Set(rustok_core::generate_id()),
                tenant_id: Set(tenant_id),
                product_id: Set(product_id),
                field_key: Set(key.clone()),
                locale: Set(locale.clone()),
                value: Set(value.clone()),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?;
        }
    }
    if !existing.is_empty() {
        Entity::delete_many()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ProductId.eq(product_id))
            .filter(Column::Id.is_in(existing.into_values().map(|row| row.id)))
            .exec(db)
            .await?;
    }
    Ok(())
}

#[tracing::instrument(skip(db))]
pub(super) async fn delete_all(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<()> {
    Entity::delete_many()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::ProductId.eq(product_id))
        .exec(db)
        .await?;
    Ok(())
}
