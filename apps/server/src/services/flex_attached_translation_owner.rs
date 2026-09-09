use async_trait::async_trait;
use flex::{
    FlexAttachedTranslationError, FlexAttachedTranslationExactLocaleApply,
    FlexAttachedTranslationExactLocaleApplyReceipt, FlexAttachedTranslationExactLocaleSnapshot,
    FlexAttachedTranslationOwnerPort, FlexAttachedTranslationResult,
    apply_flex_attached_translation_exact, lock_generic_attached_entity_write,
    read_flex_attached_translation_exact,
};
use sea_orm::{DatabaseConnection, TransactionTrait};
use uuid::Uuid;

use crate::error::Error;
use crate::services::flex_attached_values::{
    attached_ref, ensure_registered_owner_exists, load_registered_generic_schema,
};

/// Host adapter for exact Translation authoring over registered generic Flex donors.
///
/// Flex owns the schema declarations and exact localized rows. The canonical donor keeps
/// identity/lifecycle ownership; this adapter validates that owner identity before every
/// read and again under the serialized apply transaction.
#[derive(Clone)]
pub struct ServerFlexAttachedTranslationOwner {
    db: DatabaseConnection,
}

impl ServerFlexAttachedTranslationOwner {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FlexAttachedTranslationOwnerPort for ServerFlexAttachedTranslationOwner {
    async fn read_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleSnapshot> {
        ensure_registered_owner_exists(&self.db, tenant_id, entity_type, entity_id)
            .await
            .map_err(|error| owner_error(error, entity_type, entity_id))?;
        let schema = load_registered_generic_schema(&self.db, tenant_id, entity_type)
            .await
            .map_err(|error| FlexAttachedTranslationError::Database(error.to_string()))?;
        read_flex_attached_translation_exact(
            &self.db,
            attached_ref(tenant_id, entity_type, entity_id),
            &schema,
            source_locale,
            target_locale,
        )
        .await
    }

    async fn apply_exact_locale(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
        entity_id: Uuid,
        request: FlexAttachedTranslationExactLocaleApply,
    ) -> FlexAttachedTranslationResult<FlexAttachedTranslationExactLocaleApplyReceipt> {
        request.validate()?;
        let txn = self.db.begin().await.map_err(database_error)?;
        let entity = attached_ref(tenant_id, entity_type, entity_id);

        // The advisory lock is keyed by the Flex-owned donor identity and therefore also
        // protects the absent-target-row case. Canonical registered-donor GraphQL writes
        // use the same lock, so owner CAS cannot be bypassed by the ordinary write path.
        lock_generic_attached_entity_write(&txn, entity.clone())
            .await
            .map_err(|error| FlexAttachedTranslationError::Database(error.to_string()))?;
        ensure_registered_owner_exists(&txn, tenant_id, entity_type, entity_id)
            .await
            .map_err(|error| owner_error(error, entity_type, entity_id))?;
        let schema = load_registered_generic_schema(&txn, tenant_id, entity_type)
            .await
            .map_err(|error| FlexAttachedTranslationError::Database(error.to_string()))?;

        let receipt = apply_flex_attached_translation_exact(&txn, entity, &schema, &request).await?;
        txn.commit().await.map_err(database_error)?;
        Ok(receipt)
    }
}

fn owner_error(error: Error, entity_type: &str, entity_id: Uuid) -> FlexAttachedTranslationError {
    match error {
        Error::NotFound => FlexAttachedTranslationError::OwnerNotFound {
            entity_type: entity_type.to_string(),
            entity_id,
        },
        Error::BadRequest(message) | Error::Validation(message) => {
            FlexAttachedTranslationError::Invalid(message)
        }
        error => FlexAttachedTranslationError::Database(error.to_string()),
    }
}

fn database_error(error: sea_orm::DbErr) -> FlexAttachedTranslationError {
    FlexAttachedTranslationError::Database(error.to_string())
}
