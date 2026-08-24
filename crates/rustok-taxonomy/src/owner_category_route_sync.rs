use std::collections::BTreeSet;

use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::{
    SyncModuleCategoryInput, SyncModuleCategoryResult, TaxonomyError, TaxonomyResult,
    TaxonomyScopeType, TaxonomyTermKind,
    entities::{taxonomy_term, taxonomy_term_alias, taxonomy_term_translation},
    normalize_term_locale, normalize_term_route_key, sync_module_category_in_tx,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyModuleCategoryLocaleCopy {
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Load one exact localized canonical Category copy inside the caller transaction.
///
/// No fallback is applied: `None` means that exact locale is not present. The
/// Category identity must already belong to the requested tenant/module scope.
pub async fn load_module_category_locale_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
    locale: &str,
) -> TaxonomyResult<Option<TaxonomyModuleCategoryLocaleCopy>> {
    let module_scope = normalize_module_scope(module_scope)?;
    let locale = normalize_term_locale(locale)
        .ok_or_else(|| TaxonomyError::validation("Locale cannot be empty or invalid"))?;
    let term = taxonomy_term::Entity::find_by_id(category_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .one(txn)
        .await?;
    if term.is_none() {
        return Err(TaxonomyError::TermNotFound(category_id));
    }

    Ok(taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
        .one(txn)
        .await?
        .map(|translation| TaxonomyModuleCategoryLocaleCopy {
            locale: translation.locale,
            name: translation.name,
            slug: translation.slug,
            description: translation.description,
        }))
}

/// Synchronize hierarchy/presentation for an existing module Category without a
/// consumer-owned copy donor. One deterministic Taxonomy locale is replayed
/// unchanged while the shared owner-sync applies the new structure snapshot.
pub async fn sync_module_category_structure_with_owned_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
    canonical_key: String,
    parent_id: Option<Uuid>,
    position: i32,
    icon_key: Option<String>,
    color: Option<String>,
) -> TaxonomyResult<SyncModuleCategoryResult> {
    let module_scope = normalize_module_scope(module_scope)?;
    let translation = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(category_id))
        .order_by_asc(taxonomy_term_translation::Column::Locale)
        .order_by_asc(taxonomy_term_translation::Column::Id)
        .one(txn)
        .await?
        .ok_or_else(|| {
            TaxonomyError::conflict(format!(
                "Category {category_id} has no canonical localized copy for structure synchronization"
            ))
        })?;

    sync_module_category_with_owned_aliases_in_tx(
        txn,
        tenant_id,
        SyncModuleCategoryInput {
            category_id,
            module_scope,
            canonical_key,
            locale: translation.locale,
            name: translation.name,
            slug: translation.slug,
            aliases: Vec::new(),
            description: translation.description,
            parent_id,
            position,
            icon_key,
            color,
        },
    )
    .await
}

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
    aliases.extend(std::mem::take(&mut input.aliases));

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

fn normalize_module_scope(value: &str) -> TaxonomyResult<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect::<String>();
    if normalized.is_empty() {
        return Err(TaxonomyError::validation(
            "Module scope requires a non-empty scope value",
        ));
    }
    Ok(normalized)
}
