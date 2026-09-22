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

fn canonical_key_for_forum_category(category_id: Uuid) -> String {
    format!("forum-category-{category_id}")
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Forum Category Taxonomy synchronization failed: {other}"
        )),
    }
}
