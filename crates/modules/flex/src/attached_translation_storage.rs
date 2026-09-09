use std::collections::BTreeMap;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Map, Value};
use uuid::Uuid;

use rustok_api::normalize_locale_tag;
use rustok_core::field_schema::FlexError;

use crate::{
    attached::{Column, Entity},
    is_valid_flex_entity_type,
};

pub const MAX_ATTACHED_TRANSLATION_STORAGE_BATCH: usize = 200;

pub type FlexAttachedLocalizedValuesByEntity =
    BTreeMap<Uuid, BTreeMap<String, Map<String, Value>>>;

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
