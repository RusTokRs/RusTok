use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use uuid::Uuid;

use crate::{
    BlogResult, entities::translation_change::ActiveModel as TranslationChangeActiveModel,
};

pub(crate) const TRANSLATION_OWNER_SLUG: &str = "blog";
pub(crate) const TRANSLATION_RESOURCE_KIND: &str = "category";

pub(crate) struct TranslationChangeEvidence<'a> {
    pub tenant_id: Uuid,
    pub resource_kind: &'a str,
    pub resource_id: Uuid,
    pub locale: &'a str,
    pub resource_revision: i64,
    pub target_revision: i64,
    pub operation: &'a str,
    pub lifecycle: &'a str,
}

pub(crate) async fn record_translation_change_in_tx(
    transaction: &DatabaseTransaction,
    evidence: TranslationChangeEvidence<'_>,
) -> BlogResult<()> {
    TranslationChangeActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(evidence.tenant_id),
        resource_kind: Set(evidence.resource_kind.to_string()),
        resource_id: Set(evidence.resource_id),
        locale: Set(evidence.locale.to_string()),
        resource_revision: Set(evidence.resource_revision),
        target_revision: Set(evidence.target_revision),
        operation: Set(evidence.operation.to_string()),
        lifecycle: Set(evidence.lifecycle.to_string()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;

    Ok(())
}
