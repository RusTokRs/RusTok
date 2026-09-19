use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::error::{BlogError, BlogResult};

pub(crate) const BLOG_TAXONOMY_SCOPE: &str = "blog";

pub(crate) async fn load_category_locale_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> BlogResult<Option<rustok_taxonomy::TaxonomyModuleCategoryLocaleCopy>> {
    rustok_taxonomy::load_module_category_locale_copy_in_tx(
        txn,
        tenant_id,
        category_id,
        BLOG_TAXONOMY_SCOPE,
        locale,
    )
    .await
    .map_err(map_taxonomy_error)
}

pub(crate) async fn sync_category_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
) -> BlogResult<()> {
    rustok_taxonomy::sync_module_category_with_owned_aliases_in_tx(
        txn,
        tenant_id,
        rustok_taxonomy::SyncModuleCategoryInput {
            category_id,
            module_scope: BLOG_TAXONOMY_SCOPE.to_string(),
            canonical_key: canonical_key_for_blog_category(category_id),
            locale,
            name,
            slug,
            aliases: Vec::new(),
            description,
            parent_id,
            position,
            icon_key: None,
            color: None,
        },
    )
    .await
    .map(|_| ())
    .map_err(map_taxonomy_error)
}

pub(crate) fn canonical_key_for_blog_category(category_id: Uuid) -> String {
    format!("blog-category-{category_id}")
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> BlogError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => BlogError::Database(error),
        other => BlogError::Validation(format!(
            "Blog Category Taxonomy synchronization failed: {other}"
        )),
    }
}
