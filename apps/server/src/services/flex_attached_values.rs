use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde_json::Value;
use uuid::Uuid;

use flex::{
    AttachedEntityRef, delete_attached_localized_values, persist_localized_values,
    prepare_attached_values_create, prepare_attached_values_update, resolve_attached_payload,
};
use rustok_core::field_schema::{CustomFieldsSchema, FlexError};

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
        other => Err(FlexError::UnknownEntityType(other.to_string())),
    }
}
