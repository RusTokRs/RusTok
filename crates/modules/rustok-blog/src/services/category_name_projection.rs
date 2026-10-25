use std::collections::{HashMap, HashSet};

use rustok_taxonomy::{TaxonomyOwnerCategoryReader, TaxonomyScopeType};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::error::{BlogError, BlogResult};

const BLOG_TAXONOMY_SCOPE: &str = "blog";

#[derive(Clone, Debug)]
pub(crate) struct BlogCategoryProjectionInfo {
    pub name: String,
    pub slug: String,
}

/// Loads the canonical name/slug of the Categories referenced by a page of posts.
///
/// This is a **read projection**, so a damaged reference degrades instead of
/// failing the page: a post that still points at a Taxonomy Category which has no
/// canonical localized copy (a partially applied delete, a term deleted outside
/// the owner) loses `category_name`/`category_slug` and keeps every other field.
/// Failing here used to return `500` for the whole public list, the admin list,
/// the post detail and one Search projection page because of a single damaged
/// row — the opposite of the policy the tag list and the SEO scan already apply
/// ("one broken row is skipped with a warning, the page survives").
///
/// A storage failure is *not* degraded: an unreachable database is an
/// availability problem of the caller, not damaged Blog data, and must surface.
/// Ownership is still validated strictly by the write paths (`CategoryService`,
/// `CategoryCommandService`), which read through the same Taxonomy reader and
/// treat missing canonical ownership as an invariant violation.
pub(crate) async fn load_category_projections_map(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<HashMap<Uuid, BlogCategoryProjectionInfo>> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut category_ids = category_ids.to_vec();
    category_ids.sort_unstable();
    category_ids.dedup();

    let canonical = TaxonomyOwnerCategoryReader::new(db.clone())
        .load_scoped_categories_strict(
            tenant_id,
            TaxonomyScopeType::Module,
            Some(BLOG_TAXONOMY_SCOPE),
            Some(&category_ids),
            locale,
            fallback_locale,
        )
        .await
        .map_err(BlogError::from)?;

    // A duplicate identity is an owner contract breach, not per-row damage: the
    // reader must return at most one canonical copy per requested Category.
    let mut seen = HashSet::with_capacity(canonical.len());
    for category in &canonical {
        if !seen.insert(category.id) {
            return Err(BlogError::invariant(
                "Blog post Category Taxonomy projection contains duplicate identities",
            ));
        }
    }

    let requested = category_ids.len();
    let mut projections = HashMap::with_capacity(canonical.len());
    let mut degraded = Vec::new();
    for category in canonical {
        if category.available_locales.is_empty() {
            degraded.push(category.id);
            continue;
        }
        projections.insert(
            category.id,
            BlogCategoryProjectionInfo {
                name: category.name,
                slug: category.slug,
            },
        );
    }
    degraded.extend(
        category_ids
            .iter()
            .copied()
            .filter(|category_id| !projections.contains_key(category_id)),
    );
    degraded.sort_unstable();
    degraded.dedup();

    if !degraded.is_empty() {
        tracing::warn!(
            tenant_id = %tenant_id,
            requested,
            resolved = projections.len(),
            degraded = degraded.len(),
            degraded_category_ids = ?degraded,
            "Blog posts reference Categories without a canonical Taxonomy projection; \
             their category name is omitted from this page"
        );
    }

    Ok(projections)
}

pub(crate) async fn load_category_names_map(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_ids: &[Uuid],
    locale: &str,
    fallback_locale: Option<&str>,
) -> BlogResult<HashMap<Uuid, String>> {
    let projections = load_category_projections_map(
        db,
        tenant_id,
        category_ids,
        locale,
        fallback_locale,
    )
    .await?;
    Ok(projections
        .into_iter()
        .map(|(id, info)| (id, info.name))
        .collect())
}
