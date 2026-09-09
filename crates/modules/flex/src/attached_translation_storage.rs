use std::collections::BTreeMap;

use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Statement, Value as SeaOrmValue,
};
use serde_json::{Map, Value};
use uuid::Uuid;

use rustok_api::normalize_locale_tag;
use rustok_core::field_schema::FlexError;

use crate::{
    attached::{Column, Entity},
    attached_translation_changes::FLEX_ATTACHED_TRANSLATION_RESOURCE_STATE_TABLE,
    is_valid_flex_entity_type,
};

pub const MAX_ATTACHED_TRANSLATION_STORAGE_BATCH: usize = 200;

pub type FlexAttachedLocalizedValuesByEntity =
    BTreeMap<Uuid, BTreeMap<String, Map<String, Value>>>;
pub type FlexAttachedTranslationResourceRevisionsByEntity = BTreeMap<Uuid, String>;

#[derive(Debug, FromQueryResult)]
struct AttachedTranslationResourceRevisionRow {
    entity_id: Uuid,
    revision: i64,
}

/// Load exact localized Flex rows for one bounded donor inventory page.
///
/// The host supplies canonical donor identities; Flex owns the persistence query and
/// returns a normalized `(entity -> locale -> field)` map. Invalid persisted locales and
/// duplicate exact leaves fail closed instead of being silently hidden from Translation.
pub async fn load_attached_translation_localized_values<C>(
    connection: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_ids: &[Uuid],
) -> Result<FlexAttachedLocalizedValuesByEntity, FlexError>
where
    C: ConnectionTrait,
{
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_ids.len() > MAX_ATTACHED_TRANSLATION_STORAGE_BATCH {
        return Err(FlexError::Database(format!(
            "attached Translation storage batch exceeds {MAX_ATTACHED_TRANSLATION_STORAGE_BATCH} entities"
        )));
    }
    if entity_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let rows = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::EntityType.eq(entity_type))
        .filter(Column::EntityId.is_in(entity_ids.to_vec()))
        .order_by_asc(Column::EntityId)
        .order_by_asc(Column::Locale)
        .order_by_asc(Column::FieldKey)
        .all(connection)
        .await
        .map_err(|error| FlexError::Database(error.to_string()))?;

    let mut values = FlexAttachedLocalizedValuesByEntity::new();
    for row in rows {
        let locale = normalize_locale_tag(&row.locale)
            .ok_or_else(|| FlexError::InvalidLocale(row.locale.clone()))?;
        let fields = values
            .entry(row.entity_id)
            .or_default()
            .entry(locale.clone())
            .or_default();
        if fields.insert(row.field_key.clone(), row.value).is_some() {
            return Err(FlexError::Database(format!(
                "duplicate attached Translation leaf for {}/{}/{locale}/{}",
                entity_type, row.entity_id, row.field_key
            )));
        }
    }
    Ok(values)
}

/// Load the durable Flex-owned Translation resource revision for one bounded donor inventory page.
///
/// Revision state is PostgreSQL-only because it is the same durable evidence consumed by the
/// attached ChangeCursor. Missing entries are returned as missing map keys so callers can first
/// discard donor entities that have no exact source snapshot; a source-visible resource without
/// state is an owner invariant violation at the composition boundary.
pub async fn load_attached_translation_resource_revisions<C>(
    connection: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_ids: &[Uuid],
) -> Result<FlexAttachedTranslationResourceRevisionsByEntity, FlexError>
where
    C: ConnectionTrait,
{
    if tenant_id.is_nil() {
        return Err(FlexError::Database(
            "attached Translation revision tenant id must not be nil".to_string(),
        ));
    }
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    if entity_ids.len() > MAX_ATTACHED_TRANSLATION_STORAGE_BATCH {
        return Err(FlexError::Database(format!(
            "attached Translation revision batch exceeds {MAX_ATTACHED_TRANSLATION_STORAGE_BATCH} entities"
        )));
    }
    if entity_ids.iter().any(Uuid::is_nil) {
        return Err(FlexError::Database(
            "attached Translation revision resource ids must not be nil".to_string(),
        ));
    }
    if entity_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    if connection.get_database_backend() != DatabaseBackend::Postgres {
        return Err(FlexError::Database(
            "attached Translation resource revisions require PostgreSQL".to_string(),
        ));
    }

    let mut bind_values: Vec<SeaOrmValue> =
        vec![tenant_id.into(), entity_type.to_string().into()];
    let mut placeholders = Vec::with_capacity(entity_ids.len());
    for entity_id in entity_ids {
        bind_values.push((*entity_id).into());
        placeholders.push(format!("${}", bind_values.len()));
    }
    let rows = AttachedTranslationResourceRevisionRow::find_by_statement(
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            format!(
                "SELECT entity_id, revision FROM {FLEX_ATTACHED_TRANSLATION_RESOURCE_STATE_TABLE} WHERE tenant_id = $1 AND entity_type = $2 AND entity_id IN ({}) ORDER BY entity_id",
                placeholders.join(", ")
            ),
            bind_values,
        ),
    )
    .all(connection)
    .await
    .map_err(|error| FlexError::Database(error.to_string()))?;

    let mut revisions = FlexAttachedTranslationResourceRevisionsByEntity::new();
    for row in rows {
        if row.revision <= 0 {
            return Err(FlexError::Database(format!(
                "attached Translation resource {} has invalid durable revision {}",
                row.entity_id, row.revision
            )));
        }
        let revision = format!("attached:{}", row.revision);
        if revisions.insert(row.entity_id, revision).is_some() {
            return Err(FlexError::Database(format!(
                "duplicate attached Translation resource revision for {}/{}",
                entity_type, row.entity_id
            )));
        }
    }
    Ok(revisions)
}
