use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, Statement,
};
use uuid::Uuid;

use rustok_core::field_schema::{CustomFieldsSchema, FlexError};

use crate::{
    attached_definitions,
    cache_generation::FIELD_DEFINITION_CACHE_GENERATION_TABLE,
    field_definition_from_source, is_valid_flex_entity_type,
};

const FIELD_DEFINITION_CACHE_GENERATION_ID: i32 = 1;

/// Transaction-scoped serialization evidence for one attached Translation schema read.
///
/// `observed_generation` is deliberately *not* a Translation resource revision: the
/// durable cache-generation singleton is shared by every field-definition owner and may
/// advance for unrelated entity types. Its purpose here is only to provide the schema
/// serialization barrier. Translation resource revisions come from Flex-owned durable
/// per-resource state (`attached:N`) and are read separately inside the same transaction.
pub struct FlexAttachedTranslationSchemaLease {
    pub observed_generation: i64,
    pub schema: CustomFieldsSchema,
}

/// Read the exact active attached schema through any host transaction/connection.
/// Progress uses this inside its repeatable-read owner snapshot; apply uses the locking
/// variant below so no field-definition mutation can cross its CAS window.
pub async fn load_attached_translation_schema_in<C>(
    connection: &C,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<CustomFieldsSchema, FlexError>
where
    C: ConnectionTrait,
{
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }
    let rows = attached_definitions::Entity::find()
        .filter(attached_definitions::Column::TenantId.eq(tenant_id))
        .filter(attached_definitions::Column::EntityType.eq(entity_type))
        .filter(attached_definitions::Column::IsActive.eq(true))
        .order_by_asc(attached_definitions::Column::Position)
        .order_by_asc(attached_definitions::Column::FieldKey)
        .all(connection)
        .await
        .map_err(database_error)?;
    let definitions = rows
        .iter()
        .filter_map(field_definition_from_source)
        .collect();
    Ok(CustomFieldsSchema::new(definitions))
}

/// Serialize an attached Translation apply against every field-definition mutation and
/// load the exact active schema for one registered generic donor in the same transaction.
///
/// Every mutation of `flex_attached_field_definitions` already bumps the global durable
/// cache-generation row through a transaction-local trigger. Locking that row *before*
/// reading definitions therefore prevents both ordinary updates and phantom inserts from
/// committing across this schema snapshot. PostgreSQL/MySQL use `SELECT .. FOR UPDATE`;
/// SQLite uses a no-op UPDATE to acquire the equivalent write serialization point.
pub async fn lock_attached_translation_schema_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<FlexAttachedTranslationSchemaLease, FlexError> {
    if !is_valid_flex_entity_type(entity_type) {
        return Err(FlexError::UnknownEntityType(entity_type.to_string()));
    }

    let backend = txn.get_database_backend();
    if backend == DatabaseBackend::Sqlite {
        txn.execute_unprepared(&format!(
            "UPDATE {FIELD_DEFINITION_CACHE_GENERATION_TABLE} \
             SET generation = generation \
             WHERE id = {FIELD_DEFINITION_CACHE_GENERATION_ID}"
        ))
        .await
        .map_err(database_error)?;
    }

    let lock_suffix = if backend == DatabaseBackend::Sqlite {
        ""
    } else {
        " FOR UPDATE"
    };
    let generation_row = txn
        .query_one_raw(Statement::from_string(
            backend,
            format!(
                "SELECT generation FROM {FIELD_DEFINITION_CACHE_GENERATION_TABLE} \
                 WHERE id = {FIELD_DEFINITION_CACHE_GENERATION_ID}{lock_suffix}"
            ),
        ))
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            FlexError::Database(
                "Flex field-definition cache generation singleton is missing".to_string(),
            )
        })?;
    let observed_generation: i64 = generation_row
        .try_get("", "generation")
        .map_err(|error| FlexError::Database(error.to_string()))?;
    if observed_generation < 0 {
        return Err(FlexError::Database(
            "Flex field-definition cache generation must remain non-negative".to_string(),
        ));
    }

    let schema = load_attached_translation_schema_in(txn, tenant_id, entity_type).await?;
    Ok(FlexAttachedTranslationSchemaLease {
        observed_generation,
        schema,
    })
}

fn database_error(error: sea_orm::DbErr) -> FlexError {
    FlexError::Database(error.to_string())
}
