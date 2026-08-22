use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde_json::Value;
use uuid::Uuid;

use flex::{
    AttachedEntityRef, FlexMappedErrorKind, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE, delete_attached_localized_values, delete_generic_attached_values,
    map_flex_error, persist_localized_values, persist_prepared_generic_attached_values,
    prepare_attached_values_create, prepare_attached_values_update,
    prepare_generic_attached_values_update, resolve_attached_payload, resolve_generic_attached_values,
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
        ensure_registered_owner_exists(db, tenant_id, entity_type, entity_id).await?;
        persist_prepared_generic_attached_values(
            db,
            attached_ref(tenant_id, entity_type, entity_id),
            prepared,
        )
        .await
        .map_err(map_flex_host_error)
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
        ensure_registered_owner_exists(db, tenant_id, entity_type, entity_id).await?;
        delete_generic_attached_values(db, attached_ref(tenant_id, entity_type, entity_id))
            .await
            .map_err(map_flex_host_error)
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

async fn ensure_registered_owner_exists(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    entity_id: Uuid,
) -> ServerResult<()> {
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
                Error::Message(format!("Taxonomy Flex owner identity lookup failed: {error}"))
            })?;
            if exists {
                Ok(())
            } else {
                Err(Error::NotFound)
            }
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
