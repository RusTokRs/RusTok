//! Flex-owned shared payload storage for attached donors that do not expose an
//! owner metadata column.
//!
//! This store is optional. Historical donors may keep shared values in their
//! owner rows while using the same localized-value contract. A registered donor
//! such as `taxonomy.category` can instead use this table without adding an
//! untyped JSON column to its canonical owner aggregate.

use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use rustok_core::field_schema::{CustomFieldsSchema, FlexError};

use crate::{
    AttachedEntityRef, PreparedAttachedValuesWrite, delete_attached_localized_values,
    is_valid_flex_entity_type, persist_localized_values, prepare_attached_values_update,
    resolve_attached_payload,
};

pub const GENERIC_ATTACHED_VALUES_TABLE: &str = "flex_attached_values";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "flex_attached_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub data: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

pub async fn load_generic_attached_shared_values<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
) -> Result<Value, FlexError>
where
    C: ConnectionTrait,
{
    validate_entity_ref(&entity)?;
    Entity::find()
        .filter(Column::TenantId.eq(entity.tenant_id))
        .filter(Column::EntityType.eq(entity.entity_type))
        .filter(Column::EntityId.eq(entity.entity_id))
        .one(db)
        .await
        .map_err(database_error)
        .map(|row| row.map(|row| row.data).unwrap_or_else(empty_object))
}

pub async fn persist_generic_attached_shared_values<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    data: &Value,
) -> Result<(), FlexError>
where
    C: ConnectionTrait,
{
    validate_entity_ref(&entity)?;
    let data = normalized_object(data);
    let existing = Entity::find()
        .filter(Column::TenantId.eq(entity.tenant_id))
        .filter(Column::EntityType.eq(entity.entity_type))
        .filter(Column::EntityId.eq(entity.entity_id))
        .one(db)
        .await
        .map_err(database_error)?;

    if data.as_object().is_some_and(Map::is_empty) {
        if let Some(existing) = existing {
            let active: ActiveModel = existing.into();
            active.delete(db).await.map_err(database_error)?;
        }
        return Ok(());
    }

    let now = Utc::now().fixed_offset();
    if let Some(existing) = existing {
        let mut active: ActiveModel = existing.into();
        active.data = Set(data);
        active.updated_at = Set(now);
        active.update(db).await.map_err(database_error)?;
    } else {
        ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(entity.tenant_id),
            entity_type: Set(entity.entity_type.to_string()),
            entity_id: Set(entity.entity_id),
            data: Set(data),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .map_err(database_error)?;
    }
    Ok(())
}

pub async fn delete_generic_attached_values<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
) -> Result<(), FlexError>
where
    C: ConnectionTrait,
{
    validate_entity_ref(&entity)?;
    Entity::delete_many()
        .filter(Column::TenantId.eq(entity.tenant_id))
        .filter(Column::EntityType.eq(entity.entity_type))
        .filter(Column::EntityId.eq(entity.entity_id))
        .exec(db)
        .await
        .map_err(database_error)?;
    delete_attached_localized_values(
        db,
        entity.tenant_id,
        entity.entity_type,
        entity.entity_id,
    )
    .await?;
    Ok(())
}

/// Prepare an exact-locale update using Flex-owned shared storage plus the
/// existing generic localized-value store.
pub async fn prepare_generic_attached_values_update<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: CustomFieldsSchema,
    locale: &str,
    payload: Option<Value>,
) -> Result<PreparedAttachedValuesWrite, FlexError>
where
    C: ConnectionTrait,
{
    let shared = load_generic_attached_shared_values(db, entity.clone()).await?;
    prepare_attached_values_update(db, entity, schema, locale, &shared, payload).await
}

pub async fn persist_prepared_generic_attached_values<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    prepared: &PreparedAttachedValuesWrite,
) -> Result<(), FlexError>
where
    C: ConnectionTrait,
{
    if let Some(shared) = prepared.metadata.as_ref() {
        persist_generic_attached_shared_values(db, entity.clone(), shared).await?;
    } else {
        persist_generic_attached_shared_values(db, entity.clone(), &empty_object()).await?;
    }

    if let (Some(locale), Some(localized)) = (
        prepared.locale.as_deref(),
        prepared.localized_values.as_ref(),
    ) {
        persist_localized_values(
            db,
            entity.tenant_id,
            entity.entity_type,
            entity.entity_id,
            locale,
            localized,
        )
        .await?;
    }
    Ok(())
}

pub async fn resolve_generic_attached_values<C>(
    db: &C,
    entity: AttachedEntityRef<'_>,
    schema: CustomFieldsSchema,
    preferred_locale: &str,
    tenant_default_locale: &str,
) -> Result<Option<Value>, FlexError>
where
    C: ConnectionTrait,
{
    let shared = load_generic_attached_shared_values(db, entity.clone()).await?;
    resolve_attached_payload(
        db,
        entity,
        schema,
        &shared,
        preferred_locale,
        tenant_default_locale,
    )
    .await
}

fn validate_entity_ref(entity: &AttachedEntityRef<'_>) -> Result<(), FlexError> {
    if !is_valid_flex_entity_type(entity.entity_type) {
        return Err(FlexError::UnknownEntityType(entity.entity_type.to_string()));
    }
    Ok(())
}

fn normalized_object(value: &Value) -> Value {
    Value::Object(value.as_object().cloned().unwrap_or_default())
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}
