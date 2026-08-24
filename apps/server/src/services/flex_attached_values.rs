use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use serde_json::Value;
use uuid::Uuid;

use flex::{
    AttachedEntityRef, FlexMappedErrorKind, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE, delete_attached_localized_values,
    delete_generic_attached_values, map_flex_error, persist_localized_values,
    persist_prepared_generic_attached_values, prepare_attached_values_create,
    prepare_attached_values_update, prepare_generic_attached_values_update,
    resolve_attached_payload, resolve_generic_attached_values,
};
use rustok_core::field_schema::{CustomFieldsSchema, FlexError};

use crate::error::{Error, Result as ServerResult};
use crate::services::order_field_service::OrderFieldService;
use crate::services::product_field_service::ProductFieldService;
use crate::services::topic_field_service::TopicFieldService;
use crate::services::user_field_service::UserFieldService;

pub use flex::PreparedAttachedValuesWrite;

pub struct FlexAttachedValuesService;

impl FlexAttachedValuesService {
    pub async fn prepare_create(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        locale: &str,
        payload: Option<Value>,
    ) -> Result<PreparedAttachedValuesWrite, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        prepare_attached_values_create(schema, payload, locale)
    }

    pub async fn prepare_update(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        existing_metadata: &Value,
        payload: Option<Value>,
    ) -> Result<PreparedAttachedValuesWrite, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        prepare_attached_values_update(
            db,
            AttachedEntityRef {
                tenant_id,
                entity_type,
                entity_id,
            },
            schema,
            locale,
            existing_metadata,
            payload,
        )
        .await
    }

    pub async fn resolve_merged_payload(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        shared_metadata: &Value,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> Result<Option<Value>, FlexError> {
        let schema = load_schema(db, tenant_id, entity_type).await?;
        resolve_attached_payload(
            db,
            AttachedEntityRef {
                tenant_id,
                entity_type,
                entity_id,
            },
            schema,
            shared_metadata,
            preferred_locale,
            tenant_default_locale,
        )
        .await
    }

    /// Prepare an exact-locale update for a donor that uses Flex-owned generic
    /// attached storage. The owner identity is validated before any Flex row is
    /// read, so a Taxonomy Tag, foreign-tenant term, or stale UUID cannot be
    /// treated as `taxonomy.category`.
    pub async fn prepare_registered_generic_update(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        payload: Option<Value>,
    ) -> ServerResult<PreparedAttachedValuesWrite> {
        ensure_registered_owner_exists(db, tenant_id, entity_type, entity_id).await?;
        let schema = load_schema(db, tenant_id, entity_type)
            .await
            .map_err(map_flex_host_error)?;
        prepare_generic_attached_values_update(
            db,
            attached_ref(tenant_id, entity_type, entity_id),
            schema,
            locale,
            payload,
        )
        .await
        .map_err(map_flex_host_error)
    }

    pub async fn persist_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        prepared: &PreparedAttachedValuesWrite,
    ) -> ServerResult<()> {
        let txn = db.begin().await?;
        ensure_registered_owner_exists(&txn, tenant_id, entity_type, entity_id).await?;
        persist_prepared_generic_attached_values(
            &txn,
            attached_ref(tenant_id, entity_type, entity_id),
            prepared,
        )
        .await
        .map_err(map_flex_host_error)?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn resolve_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> ServerResult<Option<Value>> {
        ensure_registered_owner_exists(db, tenant_id, entity_type, entity_id).await?;
        let schema = load_schema(db, tenant_id, entity_type)
            .await
            .map_err(map_flex_host_error)?;
        resolve_generic_attached_values(
            db,
            attached_ref(tenant_id, entity_type, entity_id),
            schema,
            preferred_locale,
            tenant_default_locale,
        )
        .await
        .map_err(map_flex_host_error)
    }

    pub async fn delete_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> ServerResult<()> {
        let txn = db.begin().await?;
        ensure_registered_owner_exists(&txn, tenant_id, entity_type, entity_id).await?;
        delete_generic_attached_values(&txn, attached_ref(tenant_id, entity_type, entity_id))
            .await
            .map_err(map_flex_host_error)?;
        txn.commit().await?;
        Ok(())
    }

    pub async fn persist_localized_values<C>(
        db: &C,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        values: &Value,
    ) -> Result<(), FlexError>
    where
        C: ConnectionTrait,
    {
        persist_localized_values(db, tenant_id, entity_type, entity_id, locale, values).await
    }

    pub async fn delete_localized_values<C>(
        db: &C,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<u64, FlexError>
    where
        C: ConnectionTrait,
    {
        delete_attached_localized_values(db, tenant_id, entity_type, entity_id).await
    }
}

#[derive(Clone)]
pub struct FlexAttachedValuesGraphqlAdapter {
    db: DatabaseConnection,
}

impl FlexAttachedValuesGraphqlAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl flex::graphql::AttachedValuesGraphqlPort for FlexAttachedValuesGraphqlAdapter {
    async fn resolve_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        preferred_locale: &str,
        tenant_default_locale: &str,
    ) -> Result<Option<Value>, FlexError> {
        FlexAttachedValuesService::resolve_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            preferred_locale,
            tenant_default_locale,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))
    }

    async fn update_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        payload: Option<Value>,
    ) -> Result<Option<Value>, FlexError> {
        let prepared = FlexAttachedValuesService::prepare_registered_generic_update(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            locale,
            payload,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))?;
        FlexAttachedValuesService::persist_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            &prepared,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))?;
        FlexAttachedValuesService::resolve_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            locale,
            locale,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))
    }

    async fn delete_values(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<(), FlexError> {
        FlexAttachedValuesService::delete_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
        )
        .await
        .map_err(|error| map_host_error_to_flex(error, entity_id))
    }
}

/// Host implementation of the Taxonomy-owned Category delete cleanup boundary.
///
/// Taxonomy controls the owner transaction and calls this port before deleting the Category row;
/// the host delegates only capability-owned attached rows to Flex.
#[cfg(feature = "mod-taxonomy")]
pub struct FlexTaxonomyCategoryDeleteCleanup;

#[cfg(feature = "mod-taxonomy")]
#[async_trait::async_trait]
impl rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort for FlexTaxonomyCategoryDeleteCleanup {
    async fn cleanup_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> rustok_taxonomy::TaxonomyResult<()> {
        delete_generic_attached_values(
            txn,
            attached_ref(tenant_id, TAXONOMY_CATEGORY_ENTITY_TYPE, category_id),
        )
        .await
        .map_err(|error| {
            rustok_taxonomy::TaxonomyError::Database(sea_orm::DbErr::Custom(error.to_string()))
        })
    }
}

fn attached_ref<'a>(
    tenant_id: Uuid,
    entity_type: &'a str,
    entity_id: Uuid,
) -> AttachedEntityRef<'a> {
    AttachedEntityRef {
        tenant_id,
        entity_type,
        entity_id,
    }
}

async fn ensure_registered_owner_exists<C>(
    db: &C,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> ServerResult<()>
where
    C: ConnectionTrait,
{
    match entity_type {
        #[cfg(feature = "mod-taxonomy")]
        TAXONOMY_CATEGORY_ENTITY_TYPE => {
            let exists = rustok_taxonomy::taxonomy_term_identity_exists(
                db,
                tenant_id,
                rustok_taxonomy::TaxonomyTermKind::Category,
                entity_id,
            )
            .await
            .map_err(|error| {
                Error::Message(format!(
                    "Taxonomy Flex owner identity lookup failed: {error}"
                ))
            })?;
            if exists { Ok(()) } else { Err(Error::NotFound) }
        }
        other => Err(Error::BadRequest(format!(
            "generic Flex owner adapter is not registered for {other}"
        ))),
    }
}

fn map_flex_host_error(error: FlexError) -> Error {
    let mapped = map_flex_error(error);
    match mapped.kind {
        FlexMappedErrorKind::Internal => Error::Message(mapped.message),
        FlexMappedErrorKind::NotFound => Error::NotFound,
        FlexMappedErrorKind::BadUserInput => Error::BadRequest(mapped.message),
    }
}

fn map_host_error_to_flex(error: Error, entity_id: Uuid) -> FlexError {
    match error {
        Error::NotFound => FlexError::NotFound(entity_id),
        Error::BadRequest(message) | Error::Validation(message) => {
            FlexError::UnknownEntityType(message)
        }
        error => FlexError::Database(error.to_string()),
    }
}

async fn load_schema(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<CustomFieldsSchema, FlexError> {
    match entity_type {
        "user" => UserFieldService::get_schema(db, tenant_id).await,
        "product" => ProductFieldService::get_schema(db, tenant_id).await,
        "order" => OrderFieldService::get_schema(db, tenant_id).await,
        "topic" => TopicFieldService::get_schema(db, tenant_id).await,
        #[cfg(feature = "mod-taxonomy")]
        TAXONOMY_CATEGORY_ENTITY_TYPE => {
            GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE)
                .get_schema(db, tenant_id)
                .await
        }
        other => Err(FlexError::UnknownEntityType(other.to_string())),
    }
}
