use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use serde_json::{Map, Value};
use uuid::Uuid;

use flex::{
    AttachedEntityRef, FlexMappedErrorKind, GenericAttachedFieldDefinitionService,
    TAXONOMY_CATEGORY_ENTITY_TYPE, delete_attached_localized_values,
    delete_generic_attached_values, load_exact_locale_values, load_generic_attached_shared_values,
    load_localized_values_by_locale, lock_attached_translation_schema_in_tx, map_flex_error,
    persist_localized_values, persist_prepared_generic_attached_values,
    prepare_attached_values_create, prepare_attached_values_update,
    prepare_generic_attached_values_update, record_flex_attached_translation_deleted_in_tx,
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
    /// Canonical exact-locale mutation for a registered generic donor.
    ///
    /// Lock order is Flex schema generation -> donor owner. The write is prepared from
    /// current shared/exact values inside that transaction, persisted only on semantic
    /// change, and then advances the donor aggregate revision in the same commit. This is
    /// the mutation law shared with attached Translation CAS.
    pub async fn update_registered_generic_values(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        locale: &str,
        payload: Option<Value>,
    ) -> ServerResult<()> {
        match entity_type {
            #[cfg(feature = "mod-taxonomy")]
            TAXONOMY_CATEGORY_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let schema = lock_attached_translation_schema_in_tx(
                    &txn,
                    tenant_id,
                    entity_type,
                )
                .await
                .map_err(map_flex_host_error)?;
                let owner = rustok_taxonomy::lock_category_owner_revision_in_tx(
                    &txn,
                    tenant_id,
                    entity_id,
                )
                .await
                .map_err(map_taxonomy_owner_error)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let before_shared = load_generic_attached_shared_values(&txn, entity.clone())
                    .await
                    .map_err(map_flex_host_error)?;
                let prepared = prepare_generic_attached_values_update(
                    &txn,
                    entity.clone(),
                    schema.schema,
                    locale,
                    payload,
                )
                .await
                .map_err(map_flex_host_error)?;
                let before_localized = match prepared.locale.as_deref() {
                    Some(locale) if prepared.localized_values.is_some() => {
                        load_exact_locale_values(&txn, tenant_id, entity_type, entity_id, locale)
                            .await
                            .map_err(map_flex_host_error)?
                    }
                    _ => None,
                };
                let changed = prepared_write_changed(
                    &before_shared,
                    before_localized.as_ref(),
                    &prepared,
                );
                if changed {
                    persist_prepared_generic_attached_values(&txn, entity, &prepared)
                        .await
                        .map_err(map_flex_host_error)?;
                    rustok_taxonomy::advance_category_owner_revision_in_tx(
                        &txn,
                        tenant_id,
                        entity_id,
                        owner.revision,
                    )
                    .await
                    .map_err(map_taxonomy_owner_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            other => Err(Error::BadRequest(format!(
                "generic Flex owner adapter is not registered for {other}"
            ))),
        }
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
        match entity_type {
            #[cfg(feature = "mod-taxonomy")]
            TAXONOMY_CATEGORY_ENTITY_TYPE => {
                let txn = db.begin().await?;
                let owner = rustok_taxonomy::lock_category_owner_revision_in_tx(
                    &txn,
                    tenant_id,
                    entity_id,
                )
                .await
                .map_err(map_taxonomy_owner_error)?;
                let entity = attached_ref(tenant_id, entity_type, entity_id);
                let before_shared = load_generic_attached_shared_values(&txn, entity.clone())
                    .await
                    .map_err(map_flex_host_error)?;
                let before_localized = load_localized_values_by_locale(
                    &txn,
                    tenant_id,
                    entity_type,
                    entity_id,
                )
                .await
                .map_err(map_flex_host_error)?;
                let changed = !normalized_object(Some(&before_shared))
                    .as_object()
                    .is_some_and(Map::is_empty)
                    || !before_localized.is_empty();

                // Always clean capability rows, including any legacy-invalid locale rows
                // hidden by normalized read helpers. Only owner-visible semantic state bumps
                // the Category revision.
                delete_generic_attached_values(&txn, entity)
                    .await
                    .map_err(map_flex_host_error)?;
                if changed {
                    rustok_taxonomy::advance_category_owner_revision_in_tx(
                        &txn,
                        tenant_id,
                        entity_id,
                        owner.revision,
                    )
                    .await
                    .map_err(map_taxonomy_owner_error)?;
                }
                txn.commit().await?;
                Ok(())
            }
            other => Err(Error::BadRequest(format!(
                "generic Flex owner adapter is not registered for {other}"
            ))),
        }
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
        FlexAttachedValuesService::update_registered_generic_values(
            &self.db,
            tenant_id,
            entity_type,
            entity_id,
            locale,
            payload,
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
        })?;
        record_flex_attached_translation_deleted_in_tx(
            txn,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
        )
        .await
        .map_err(|error| {
            rustok_taxonomy::TaxonomyError::Database(sea_orm::DbErr::Custom(error.to_string()))
        })?;
        Ok(())
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

fn prepared_write_changed(
    before_shared: &Value,
    before_localized: Option<&Value>,
    prepared: &PreparedAttachedValuesWrite,
) -> bool {
    let desired_shared = normalized_object(prepared.metadata.as_ref());
    if normalized_object(Some(before_shared)) != desired_shared {
        return true;
    }
    match prepared.localized_values.as_ref() {
        Some(desired) => normalized_object(before_localized) != normalized_object(Some(desired)),
        None => false,
    }
}

fn normalized_object(value: Option<&Value>) -> Value {
    Value::Object(
        value
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    )
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

#[cfg(feature = "mod-taxonomy")]
fn map_taxonomy_owner_error(error: rustok_taxonomy::TaxonomyError) -> Error {
    match error {
        rustok_taxonomy::TaxonomyError::TermNotFound(_) => Error::NotFound,
        rustok_taxonomy::TaxonomyError::Validation(message) => Error::BadRequest(message),
        rustok_taxonomy::TaxonomyError::Database(error) => Error::Database(error),
        error => Error::Message(format!("Taxonomy Flex owner mutation failed: {error}")),
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
