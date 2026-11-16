use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortActorKind, PortContext, PortError, normalize_locale_tag};
use rustok_core::field_schema::CustomFieldsSchema;
use rustok_events::DomainEvent;
use rustok_product_ports::attached_values::validate_attached_values_patch;
use rustok_product_ports::{ProductAttachedValuesReadPort, ProductAttachedValuesWritePort};
use sea_orm::{
    AccessMode, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    IsolationLevel, QueryFilter, Set, TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;

use super::attached_localized_storage;
use super::commands::find_product_for_update_in_tx;
use super::concurrency::next_revision;
use super::helpers::resolve_product_attached_payload;
use crate::catalog_command_port::execute_receipted_catalog_command;
use crate::services::flex_schema_projection::ProductFlexSchemaSource;
use crate::services::write_transaction::{
    ProductWriteTransaction, record_product_operation_result,
};
use crate::{CatalogService, CommerceError, CommerceResult, entities::product};

const PRODUCT_OWNER_METADATA_KEYS: &[&str] = &["tags", "shipping_profile", "shipping_profile_slug"];

#[async_trait]
impl ProductAttachedValuesReadPort for CatalogService {
    async fn read_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        fallback_locale: &str,
    ) -> Result<Option<Value>, PortError> {
        context.require_read_semantics()?;
        let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation("product.tenant_id_invalid", "Product tenant is invalid")
        })?;
        validate_product_identity(tenant_id, product_id)?;
        let snapshot = match self.db.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::RepeatableRead),
                        Some(AccessMode::ReadOnly),
                    )
                    .await
            }
            _ => self.db.begin().await,
        }
        .map_err(|error| attached_values_error(error.into()))?;
        let row = product::Entity::find_by_id(product_id)
            .filter(product::Column::TenantId.eq(tenant_id))
            .one(&snapshot)
            .await
            .map_err(|error| attached_values_error(error.into()))?
            .ok_or_else(|| attached_values_error(CommerceError::ProductNotFound(product_id)))?;
        let schema = load_attached_fields_schema(&snapshot, self.flex_schema_source, tenant_id)
            .await
            .map_err(attached_values_error)?;
        let values = resolve_values(
            &snapshot,
            tenant_id,
            product_id,
            schema,
            &row.metadata,
            &context.locale,
            fallback_locale,
        )
        .await
        .map_err(attached_values_error)?;
        snapshot
            .commit()
            .await
            .map_err(|error| attached_values_error(error.into()))?;
        Ok(values)
    }
}

#[async_trait]
impl ProductAttachedValuesWritePort for CatalogService {
    async fn update_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
        values: Option<Value>,
    ) -> Result<Option<Value>, PortError> {
        let (tenant_id, actor_id) = attached_write_scope(&context, product_id)?;
        validate_attached_values_patch(values.as_ref())?;
        let locale = normalize_locale_tag(&context.locale).ok_or_else(|| {
            PortError::validation(
                "product.attached_values_invalid",
                "Custom field locale is invalid",
            )
        })?;
        let request = serde_json::json!({
            "actor_kind": context.actor.kind,
            "actor_id": actor_id,
            "product_id": product_id,
            "locale": locale,
            "values": values,
        });
        execute_receipted_catalog_command(
            self,
            &context,
            tenant_id,
            "update_attached_values",
            &request,
            self.update_attached_values_in_owner(
                tenant_id,
                event_actor_id(&context, actor_id),
                product_id,
                &locale,
                values,
            ),
            |error| error.0,
        )
        .await
    }

    async fn delete_attached_values(
        &self,
        context: PortContext,
        product_id: Uuid,
    ) -> Result<(), PortError> {
        let (tenant_id, actor_id) = attached_write_scope(&context, product_id)?;
        let request = serde_json::json!({
            "actor_kind": context.actor.kind,
            "actor_id": actor_id,
            "product_id": product_id,
        });
        execute_receipted_catalog_command(
            self,
            &context,
            tenant_id,
            "delete_attached_values",
            &request,
            self.delete_attached_values_in_owner(
                tenant_id,
                event_actor_id(&context, actor_id),
                product_id,
            ),
            |error| error.0,
        )
        .await
    }
}

impl CatalogService {
    async fn update_attached_values_in_owner(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        product_id: Uuid,
        locale: &str,
        values: Option<Value>,
    ) -> Result<Option<Value>, AttachedValuesWriteError> {
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone())
            .await
            .map_err(attached_values_error)?;
        let row = find_product_for_update_in_tx(&txn, tenant_id, product_id)
            .await
            .map_err(attached_values_error)?;
        let schema = load_attached_fields_schema(&*txn, self.flex_schema_source, tenant_id)
            .await
            .map_err(attached_values_error)?;
        if let Some(values) = values.as_ref().and_then(Value::as_object) {
            let definitions = schema.active_definitions();
            if values
                .keys()
                .any(|key| !definitions.iter().any(|field| field.field_key == *key))
            {
                return Err(PortError::validation(
                    "product.attached_field_unknown",
                    "Product custom field is not defined",
                )
                .into());
            }
        }

        let (reserved, current_values) = flex::split_donor_metadata(&schema, &row.metadata);
        let existing_localized =
            attached_localized_storage::load_exact(&*txn, tenant_id, product_id, locale)
                .await
                .map_err(attached_values_input_error)?;
        let prepared = flex::prepare_attached_values_update_from_values(
            schema.clone(),
            locale,
            &Value::Object(current_values),
            existing_localized,
            values,
        )
        .map_err(|error| attached_values_input_error(error.into()))?;
        let metadata = flex::merge_reserved_donor_metadata(reserved, prepared.metadata);
        save_product_metadata(&*txn, row, metadata.clone())
            .await
            .map_err(attached_values_error)?;

        if let (Some(locale), Some(localized)) = (prepared.locale, prepared.localized_values) {
            attached_localized_storage::replace_for_locale(
                &*txn, tenant_id, product_id, &locale, &localized,
            )
            .await
            .map_err(attached_values_error)?;
        }
        let resolved = resolve_values(
            &*txn, tenant_id, product_id, schema, &metadata, locale, locale,
        )
        .await
        .map_err(attached_values_error)?;
        publish_product_update(txn, tenant_id, actor_id, product_id, &resolved).await?;
        Ok(resolved)
    }

    async fn delete_attached_values_in_owner(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        product_id: Uuid,
    ) -> Result<(), AttachedValuesWriteError> {
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone())
            .await
            .map_err(attached_values_error)?;
        let row = find_product_for_update_in_tx(&txn, tenant_id, product_id)
            .await
            .map_err(attached_values_error)?;
        let schema = load_attached_fields_schema(&*txn, self.flex_schema_source, tenant_id)
            .await
            .map_err(attached_values_error)?;
        let (reserved, _) = flex::split_donor_metadata(&schema, &row.metadata);
        save_product_metadata(&*txn, row, Value::Object(reserved))
            .await
            .map_err(attached_values_error)?;
        attached_localized_storage::delete_all(&*txn, tenant_id, product_id)
            .await
            .map_err(attached_values_error)?;
        publish_product_update(txn, tenant_id, actor_id, product_id, &()).await?;
        Ok(())
    }
}

struct AttachedValuesWriteError(PortError);

impl From<CommerceError> for AttachedValuesWriteError {
    fn from(error: CommerceError) -> Self {
        Self(attached_values_error(error))
    }
}

impl From<PortError> for AttachedValuesWriteError {
    fn from(error: PortError) -> Self {
        Self(error)
    }
}

fn attached_write_scope(
    context: &PortContext,
    product_id: Uuid,
) -> Result<(Uuid, Uuid), PortError> {
    context.require_write_semantics()?;
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation("product.tenant_id_invalid", "Product tenant is invalid")
    })?;
    validate_product_identity(tenant_id, product_id)?;
    let actor_id = Uuid::parse_str(&context.actor.id)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            PortError::validation("product.actor_id_invalid", "Product actor is invalid")
        })?;
    Ok((tenant_id, actor_id))
}

fn event_actor_id(context: &PortContext, actor_id: Uuid) -> Option<Uuid> {
    (context.actor.kind == PortActorKind::User).then_some(actor_id)
}

fn validate_product_identity(tenant_id: Uuid, product_id: Uuid) -> Result<(), PortError> {
    if tenant_id.is_nil() || product_id.is_nil() {
        return Err(PortError::validation(
            "product.attached_identity_invalid",
            "Product attached values require tenant and product identities",
        ));
    }
    Ok(())
}

async fn load_attached_fields_schema(
    db: &impl ConnectionTrait,
    source: ProductFlexSchemaSource,
    tenant_id: Uuid,
) -> CommerceResult<CustomFieldsSchema> {
    let schema = source.load(db, tenant_id).await?;
    let definitions = schema
        .active_definitions()
        .into_iter()
        .filter(|field| !PRODUCT_OWNER_METADATA_KEYS.contains(&field.field_key.as_str()))
        .cloned()
        .collect();
    Ok(CustomFieldsSchema::new(definitions))
}

async fn resolve_values(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    product_id: Uuid,
    schema: CustomFieldsSchema,
    metadata: &Value,
    preferred_locale: &str,
    fallback_locale: &str,
) -> CommerceResult<Option<Value>> {
    let (_, values) = flex::split_donor_metadata(&schema, metadata);
    let resolved = resolve_product_attached_payload(
        db,
        tenant_id,
        product_id,
        &Value::Object(values),
        preferred_locale,
        fallback_locale,
        &schema,
    )
    .await?;
    let Some(Value::Object(resolved)) = resolved else {
        return Ok(None);
    };
    let (_, values) = flex::split_donor_metadata(&schema, &Value::Object(resolved));
    Ok((!values.is_empty()).then_some(Value::Object(values)))
}

async fn save_product_metadata(
    db: &impl ConnectionTrait,
    row: product::Model,
    metadata: Value,
) -> CommerceResult<()> {
    let revision = next_revision(row.revision)?;
    let mut active: product::ActiveModel = row.into();
    active.metadata = Set(metadata);
    active.revision = Set(revision);
    active.updated_at = Set(Utc::now().into());
    active.update(db).await?;
    Ok(())
}

async fn publish_product_update(
    txn: ProductWriteTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    product_id: Uuid,
    response: &impl serde::Serialize,
) -> Result<(), PortError> {
    txn.publish(
        tenant_id,
        actor_id,
        DomainEvent::ProductUpdated { product_id },
    )
    .await
    .map_err(attached_values_error)?;
    record_product_operation_result(response);
    txn.commit().await.map_err(attached_values_error)
}

fn attached_values_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::ProductNotFound(_) => {
            PortError::not_found("product.not_found", "Product not found")
        }
        CommerceError::Core(error) => crate::error::core_port_error(&error),
        CommerceError::Database(_) => PortError::unavailable(
            "product.database_unavailable",
            "Product storage is temporarily unavailable",
        ),
        _ => PortError::invariant_violation(
            "product.attached_values_failed",
            "Product attached values operation failed",
        ),
    }
}

fn attached_values_input_error(error: CommerceError) -> PortError {
    match error {
        CommerceError::Validation(message) => {
            PortError::validation("product.attached_values_invalid", message)
        }
        error => attached_values_error(error),
    }
}

#[cfg(test)]
#[path = "../../tests/attached_values.rs"]
mod tests;
