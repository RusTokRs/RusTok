use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

const FORUM_TAXONOMY_SCOPE: &str = "forum";

pub(in crate::services) async fn load_category_locale_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<Option<rustok_taxonomy::TaxonomyModuleCategoryLocaleCopy>> {
    rustok_taxonomy::load_module_category_locale_copy_in_tx(
        txn,
        tenant_id,
        category_id,
        FORUM_TAXONOMY_SCOPE,
        locale,
    )
    .await
    .map_err(map_taxonomy_error)
}

pub(in crate::services) async fn sync_category_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    icon_key: Option<String>,
    color: Option<String>,
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
) -> ForumResult<()> {
    rustok_taxonomy::sync_module_category_with_owned_aliases_in_tx(
        txn,
        tenant_id,
        rustok_taxonomy::SyncModuleCategoryInput {
            category_id,
            module_scope: FORUM_TAXONOMY_SCOPE.to_string(),
            canonical_key: canonical_key_for_forum_category(category_id),
            locale,
            name,
            slug,
            aliases: Vec::new(),
            description,
            parent_id,
            position,
            icon_key,
            color,
        },
    )
    .await
    .map(|_| ())
    .map_err(map_taxonomy_error)
}

pub(in crate::services) async fn load_category_owner_snapshot_in_tx(
    txn: &DatabaseTransaction, tenant_id: Uuid, category_id: Uuid, locale: &str,
) -> ForumResult<rustok_taxonomy::TaxonomyOwnerCategory> {
    rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id)
        .await.map_err(map_taxonomy_error)?;
    let categories = rustok_taxonomy::TaxonomyOwnerCategoryReader::load_scoped_categories_in_strict(
        txn, tenant_id, rustok_taxonomy::TaxonomyScopeType::Module, Some(FORUM_TAXONOMY_SCOPE),
        Some(&[category_id]), locale, None,
    ).await.map_err(map_taxonomy_error)?;
    categories.into_iter().next().ok_or(ForumError::CategoryNotFound(category_id))
}

pub(in crate::services) async fn load_category_siblings_in_tx(
    txn: &DatabaseTransaction, tenant_id: Uuid, parent_id: Option<Uuid>,
) -> ForumResult<Vec<Uuid>> {
    rustok_taxonomy::TaxonomyOwnerCategoryReader::load_module_category_sibling_ids_in(
        txn, tenant_id, FORUM_TAXONOMY_SCOPE, parent_id,
    ).await.map_err(map_taxonomy_error)
}

pub(in crate::services) async fn shift_category_siblings_for_insert_in_tx(
    txn: &DatabaseTransaction, tenant_id: Uuid, category_id: Uuid, parent_id: Option<Uuid>, requested_position: i32,
) -> ForumResult<()> {
    rustok_taxonomy::shift_module_category_siblings_for_insert_in_tx(
        txn, tenant_id, category_id, FORUM_TAXONOMY_SCOPE, parent_id, requested_position,
    ).await.map_err(map_taxonomy_error)
}

pub(in crate::services) async fn move_category_in_tx(
    txn: &DatabaseTransaction, tenant_id: Uuid, category_id: Uuid, parent_id: Option<Uuid>, position: i32,
) -> ForumResult<Vec<crate::dto::CategoryPlacementResponse>> {
    rustok_taxonomy::move_module_category_in_tx(
        txn, tenant_id, category_id, FORUM_TAXONOMY_SCOPE, parent_id, position,
    ).await.map(|placements| placements.into_iter().map(|p| crate::dto::CategoryPlacementResponse {
        id: p.term_id, parent_id: p.parent_id, position: p.position,
    }).collect()).map_err(map_taxonomy_error)
}

pub(in crate::services) async fn reorder_category_siblings_in_tx(
    txn: &DatabaseTransaction, tenant_id: Uuid, parent_id: Option<Uuid>, ordered_ids: &[Uuid],
) -> ForumResult<()> {
    rustok_taxonomy::reorder_module_category_siblings_in_tx(
        txn, tenant_id, FORUM_TAXONOMY_SCOPE, parent_id, ordered_ids,
    ).await.map_err(map_taxonomy_error)
}
fn canonical_key_for_forum_category(category_id: Uuid) -> String {
    format!("forum-category-{category_id}")
}

pub(in crate::services) fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        rustok_taxonomy::TaxonomyError::TermNotFound(category_id) => ForumError::CategoryNotFound(category_id),
        other => ForumError::Validation(format!(
            "Forum Category Taxonomy synchronization failed: {other}"
        )),
    }
}
