use std::collections::BTreeSet;

use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    SyncModuleCategoryInput, SyncModuleCategoryResult, TaxonomyError, TaxonomyResult,
    entities::{taxonomy_term_alias, taxonomy_term_translation}, normalize_term_locale,
    normalize_term_route_key, sync_module_category_in_tx,
};

/// Synchronize one module Category while Taxonomy owns append-only route history.
///
/// Existing Taxonomy aliases are retained automatically and a changed canonical
/// localized slug is appended as a historical alias before the normal owner-sync
/// validates and reconciles the route registry. Caller-provided aliases remain
/// additive bootstrap input for migrations, but consumers no longer need to keep
/// a duplicate alias ledger merely to submit a complete snapshot on every write.
pub async fn sync_module_category_with_owned_aliases_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    mut input: SyncModuleCategoryInput,
) -> TaxonomyResult<SyncModuleCategoryResult> {
    let locale = normalize_term_locale(&input.locale)
        .ok_or_else(|| TaxonomyError::validation("Locale cannot be empty or invalid"))?;
    let next_slug = normalize_term_route_key(&input.slug)
        .ok_or_else(|| TaxonomyError::validation("Category localized slug cannot be empty"))?;

    let mut aliases = taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::TermId.eq(input.category_id))
        .filter(taxonomy_term_alias::Column::Locale.eq(&locale))
        .all(txn)
        .await?
        .into_iter()
        .map(|alias| alias.slug)
        .collect::<BTreeSet<_>>();
    aliases.extend(input.aliases);

    if let Some(existing) = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(input.category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
        .one(txn)
        .await?
    {
        let previous_slug = normalize_term_route_key(&existing.slug).ok_or_else(|| {
            TaxonomyError::conflict(format!(
                "Category {} has an invalid stored localized route key for locale {locale}",
                input.category_id
            ))
        })?;
        if previous_slug != next_slug {
            aliases.insert(previous_slug);
        }
    }

    input.aliases = aliases.into_iter().collect();
    sync_module_category_in_tx(txn, tenant_id, input).await
}
