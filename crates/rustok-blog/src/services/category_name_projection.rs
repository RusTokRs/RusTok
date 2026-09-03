use std::collections::HashMap;

use rustok_taxonomy::{TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::blog_category_taxonomy_binding;
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

    let bindings = blog_category_taxonomy_binding::Entity::find()
        .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .filter(blog_category_taxonomy_binding::Column::BlogCategoryId.is_in(category_ids.clone()))
        .all(db)
        .await?;
    if bindings.len() != category_ids.len() {
        return Err(BlogError::validation(
            "Blog post Category Taxonomy binding coverage is incomplete",
        ));
    }

    let binding_by_blog = bindings
        .iter()
        .map(|binding| (binding.blog_category_id, binding.taxonomy_category_id))
        .collect::<HashMap<_, _>>();
    let blog_by_taxonomy = bindings
        .iter()
        .map(|binding| (binding.taxonomy_category_id, binding.blog_category_id))
        .collect::<HashMap<_, _>>();
    if binding_by_blog.len() != category_ids.len() || blog_by_taxonomy.len() != category_ids.len() {
        return Err(BlogError::validation(
            "Blog post Category Taxonomy binding is not one-to-one",
        ));
    }

    let taxonomy_ids = bindings
        .iter()
        .map(|binding| binding.taxonomy_category_id)
        .collect::<Vec<_>>();
    let canonical = TaxonomyOwnerCategoryReader::new(db.clone())
        .load_scoped_categories(
            tenant_id,
            TaxonomyScopeType::Module,
            Some(BLOG_TAXONOMY_SCOPE),
            Some(&taxonomy_ids),
            locale,
            fallback_locale,
        )
        .await
        .map_err(map_taxonomy_error)?;
    if canonical.len() != category_ids.len() {
        return Err(BlogError::validation(
            "Blog post Category Taxonomy projection coverage is incomplete",
        ));
    }

    let canonical_by_id = canonical
        .into_iter()
        .map(|category| (category.id, category))
        .collect::<HashMap<_, _>>();
    if canonical_by_id.len() != category_ids.len() {
        return Err(BlogError::validation(
            "Blog post Category Taxonomy projection contains duplicate identities",
        ));
    }

    category_ids
        .into_iter()
        .map(|blog_category_id| {
            let taxonomy_category_id =
                binding_by_blog
                    .get(&blog_category_id)
                    .copied()
                    .ok_or_else(|| {
                        BlogError::validation(format!(
                            "Blog category {blog_category_id} has no Taxonomy binding"
                        ))
                    })?;
            let canonical = canonical_by_id.get(&taxonomy_category_id).ok_or_else(|| {
                BlogError::validation(format!(
                    "Blog category {blog_category_id} Taxonomy projection is missing"
                ))
            })?;
            Ok((blog_category_id, canonical.name.clone()))
        })
        .collect()
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> BlogError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => BlogError::Database(error),
        other => BlogError::Validation(format!(
            "Blog post Category Taxonomy projection failed: {other}"
        )),
    }
}
