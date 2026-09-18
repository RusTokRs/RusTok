use std::collections::HashMap;

use rustok_taxonomy::{TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::error::{BlogError, BlogResult};

const BLOG_TAXONOMY_SCOPE: &str = "blog";

pub(in crate::services) async fn load_category_names_map(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<HashMap<Uuid, String>> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut category_ids = category_ids.to_vec();
    category_ids.sort_unstable();
    category_ids.dedup();

    let canonical = TaxonomyOwnerCategoryReader::new(db.clone())
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some(BLOG_TAXONOMY_SCOPE),
            Some(&category_ids),
            locale,
            fallback_locale,
        )
        .await
        .map_err(BlogError::from)?;
    if canonical.len() != category_ids.len() {
        return Err(BlogError::invariant(
            "Blog post Category Taxonomy projection coverage is incomplete",
        ));
    }

    let canonical_by_id = canonical
        .into_iter()
        .map(|category| (category.id, category))
        .collect::<HashMap<_, _>>();
    if canonical_by_id.len() != category_ids.len() {
        return Err(BlogError::invariant(
            "Blog post Category Taxonomy projection contains duplicate identities",
        ));
    }

    category_ids
        .into_iter()
        .map(|category_id| {
            let canonical = canonical_by_id.get(&category_id).ok_or_else(|| {
                BlogError::invariant(format!(
                    "Blog category {category_id} Taxonomy projection is missing"
                ))
            })?;
            Ok((category_id, canonical.name.clone()))
        })
        .collect()
}

