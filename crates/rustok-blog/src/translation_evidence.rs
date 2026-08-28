use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set,
};
use uuid::Uuid;

use crate::{
    BlogError, BlogResult,
    entities::{blog_category_translation, translation_change::ActiveModel as TranslationChangeActiveModel},
};

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

    if evidence.resource_kind == TRANSLATION_RESOURCE_KIND
        && evidence.operation == "upsert"
        && evidence.lifecycle == "active"
    {
        let translation = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::TenantId.eq(evidence.tenant_id))
            .filter(blog_category_translation::Column::CategoryId.eq(evidence.resource_id))
            .filter(blog_category_translation::Column::Locale.eq(evidence.locale))
            .one(transaction)
            .await?
            .ok_or_else(|| {
                BlogError::Validation(format!(
                    "Category translation evidence cannot synchronize missing locale {} for category {}",
                    evidence.locale, evidence.resource_id
                ))
            })?;

        crate::services::category_taxonomy_sync::sync_category_copy_in_tx(
            transaction,
            evidence.tenant_id,
            evidence.resource_id,
            translation.locale,
            translation.name,
            translation.slug,
            translation.description,
        )
        .await?;
    }

    Ok(())
}
