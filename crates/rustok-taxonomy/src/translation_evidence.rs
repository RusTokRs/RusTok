use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, Set};
use uuid::Uuid;

use crate::{
    TaxonomyResult, entities::translation_change::ActiveModel as TranslationChangeActiveModel,
    route_key_registry::reconcile_route_keys_for_locale_in_tx,
};

pub(crate) const TRANSLATION_OWNER_SLUG: &str = "taxonomy";
pub(crate) const TRANSLATION_RESOURCE_KIND: &str = "term";

pub(crate) struct TranslationChangeEvidence<'a> {
    pub tenant_id: Uuid,
    pub term_id: Uuid,
    pub locale: &'a str,
    pub resource_revision: i64,
    pub target_revision: i64,
    pub operation: &'a str,
}

/// Finalizes one localized Taxonomy mutation inside the caller's transaction.
///
/// Route-key reservations are reconciled before durable change evidence is
/// appended, so a uniqueness conflict aborts the same transaction that changed
/// the translation or aliases. Deletes skip reconciliation because the
/// composite route-key rows are removed by the term foreign-key cascade.
/// Persisted terms have no soft lifecycle state: non-delete changes are active,
/// while an actual term deletion is recorded as deleted.
pub(crate) async fn record_translation_change_in_tx(
    transaction: &DatabaseTransaction,
    evidence: TranslationChangeEvidence<'_>,
) -> TaxonomyResult<()> {
    if evidence.operation != "delete" {
        reconcile_route_keys_for_locale_in_tx(
            transaction,
            evidence.tenant_id,
            evidence.term_id,
            evidence.locale,
        )
        .await?;
    }

    let lifecycle = if evidence.operation == "delete" {
        "deleted"
    } else {
        "active"
    };

    TranslationChangeActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(evidence.tenant_id),
        term_id: Set(evidence.term_id),
        locale: Set(evidence.locale.to_string()),
        resource_revision: Set(evidence.resource_revision),
        target_revision: Set(evidence.target_revision),
        operation: Set(evidence.operation.to_string()),
        lifecycle: Set(lifecycle.to_string()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(transaction)
    .await?;

    Ok(())
}
