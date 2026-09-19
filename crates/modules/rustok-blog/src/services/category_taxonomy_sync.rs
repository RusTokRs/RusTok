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
    .map_err(BlogError::from)
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
    .map_err(BlogError::from)
}

pub(crate) fn canonical_key_for_blog_category(category_id: Uuid) -> String {
    format!("blog-category-{category_id}")
}


#[cfg(test)]
mod tests {
    use super::*;
    use rustok_taxonomy::TaxonomyError;

    #[test]
    fn taxonomy_sync_preserves_typed_dependency_error_mapping() {
        let missing = BlogError::from(TaxonomyError::TermNotFound(Uuid::from_u128(1)));
        assert!(matches!(missing, BlogError::TaxonomyTermNotFound(_)));

        let conflict = BlogError::from(TaxonomyError::Conflict("conflict".to_string()));
        assert!(matches!(conflict, BlogError::Conflict(_)));

        let internal = BlogError::from(TaxonomyError::Internal("storage invariant".to_string()));
        assert!(matches!(internal, BlogError::Invariant(_)));
    }
}
