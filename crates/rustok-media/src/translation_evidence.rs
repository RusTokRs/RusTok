use chrono::Utc;
use rustok_core::generate_id;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use uuid::Uuid;

use crate::{
    MediaError, Result, entities::translation_change::ActiveModel as TranslationChangeActiveModel,
};

pub(crate) const TRANSLATION_OWNER_SLUG: &str = "media";
pub(crate) const TRANSLATION_RESOURCE_KIND: &str = "asset";

pub(crate) struct TranslationChangeEvidence<'a> {
    pub tenant_id: Uuid,
    pub media_id: Uuid,
    pub locale: &'a str,
    pub resource_revision: &'a str,
    pub target_revision: i64,
    pub operation: &'a str,
    pub lifecycle: &'a str,
    pub actor_id: Option<Uuid>,
    pub correlation_id: String,
}

pub(crate) async fn record_translation_change_in_transaction(
    transaction: &DatabaseTransaction,
    event_bus: &TransactionalEventBus,
    evidence: TranslationChangeEvidence<'_>,
) -> Result<()> {
    TranslationChangeActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(evidence.tenant_id),
        asset_id: Set(evidence.media_id),
        locale: Set(evidence.locale.to_string()),
        resource_revision: Set(evidence.resource_revision.to_string()),
        target_revision: Set(evidence.target_revision),
        operation: Set(evidence.operation.to_string()),
        lifecycle: Set(evidence.lifecycle.to_string()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;

    event_bus
        .publish_in_tx(
            transaction,
            evidence.tenant_id,
            evidence.actor_id,
            DomainEvent::TranslationTargetChanged {
                owner_slug: TRANSLATION_OWNER_SLUG.to_string(),
                resource_kind: TRANSLATION_RESOURCE_KIND.to_string(),
                resource_id: evidence.media_id.to_string(),
                changed_locale: evidence.locale.to_string(),
                resource_revision: evidence.resource_revision.to_string(),
                target_revision: evidence.target_revision.to_string(),
                operation: evidence.operation.to_string(),
                correlation_id: evidence.correlation_id,
            },
        )
        .await
        .map_err(|error| MediaError::TranslationEvent(error.to_string()))?;

    Ok(())
}
