use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{BlogError, BlogResult, entities::blog_category_translation};

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

/// Transitional Category mirror bridge after retirement of the duplicate Blog Translation
/// provider. Blog no longer writes `blog_translation_changes`; active compatibility mirror
/// upserts are synchronously replayed into canonical Taxonomy in the same owner transaction.
/// Delete journal evidence is retired because canonical Category deletion is already owned by
/// Taxonomy's module-owner lifecycle. Remove this shim together with the compatibility mirror.
pub(crate) async fn record_translation_change_in_tx(
    transaction: &DatabaseTransaction,
    evidence: TranslationChangeEvidence<'_>,
) -> BlogResult<()> {
    if evidence.resource_kind != TRANSLATION_RESOURCE_KIND {
        return Err(BlogError::validation(format!(
            "Unsupported Blog Translation compatibility resource kind {}",
            evidence.resource_kind
        )));
    }
    if evidence.resource_revision <= 0 || evidence.target_revision < 0 {
        return Err(BlogError::validation(
            "Blog Category compatibility revisions must remain non-negative",
        ));
    }

    if evidence.operation == "upsert" && evidence.lifecycle == "active" {
        if evidence.target_revision == 0 {
            return Err(BlogError::validation(
                "Active Blog Category compatibility copy requires a positive target revision",
            ));
        }
        let translation = blog_category_translation::Entity::find()
            .filter(blog_category_translation::Column::TenantId.eq(evidence.tenant_id))
            .filter(blog_category_translation::Column::CategoryId.eq(evidence.resource_id))
            .filter(blog_category_translation::Column::Locale.eq(evidence.locale))
            .one(transaction)
            .await?
            .ok_or_else(|| {
                BlogError::Validation(format!(
                    "Category compatibility copy cannot synchronize missing locale {} for category {}",
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
        return Ok(());
    }

    if evidence.operation == "delete" && evidence.lifecycle == "deleted" {
        return Ok(());
    }

    Err(BlogError::validation(format!(
        "Unsupported Blog Category compatibility transition {}/{}",
        evidence.operation, evidence.lifecycle
    )))
}
