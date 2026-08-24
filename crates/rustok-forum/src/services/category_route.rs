use rustok_content::normalize_locale_code;
use rustok_taxonomy::{
    TaxonomyOwnerCategoryReader, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_lifecycle, forum_category_taxonomy_binding,
};
use crate::error::{ForumError, ForumResult};

pub const MAX_FORUM_CATEGORY_ROUTE_LOCALE_LEN: usize = 64;
pub const MAX_FORUM_CATEGORY_ROUTE_SLUG_LEN: usize = 255;
pub const MAX_FORUM_CATEGORY_ROUTE_CANDIDATES: u64 = 64;
const FORUM_TAXONOMY_SCOPE: &str = "forum";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForumCategoryRouteDisposition {
    Canonical,
    Redirect,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumCategoryRouteDescriptor {
    pub category_id: Uuid,
    pub locale: String,
    pub slug: String,
    pub path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForumCategoryRouteResolution {
    pub requested_locale: String,
    pub requested_slug: String,
    pub disposition: ForumCategoryRouteDisposition,
    pub canonical: ForumCategoryRouteDescriptor,
    pub alias_id: Option<Uuid>,
}

/// Forum route facade for Taxonomy-owned Category route identity.
///
/// Canonical localized slugs and immutable aliases are resolved through the
/// Taxonomy route-key registry. Forum retains ownership of route disclosure:
/// the resolved Taxonomy Category must map through the same-tenant typed Forum
/// binding and the Forum category must remain active. The public path remains
/// `/{locale}/forum/c/{slug}` and hierarchy is intentionally absent from it.
pub struct ForumCategoryRouteService {
    db: DatabaseConnection,
}

impl ForumCategoryRouteService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn canonical_descriptor(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        requested_locale: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryRouteDescriptor> {
        let requested_locale = normalize_route_locale(requested_locale)?;
        let fallback_locale = fallback_locale.map(normalize_route_locale).transpose()?;
        ensure_active_category(&self.db, tenant_id, category_id).await?;

        let binding = load_taxonomy_binding_for_forum(&self.db, tenant_id, category_id).await?;
        let taxonomy_category_id = binding
            .map(|binding| binding.taxonomy_category_id)
            .ok_or(ForumError::CategoryRouteResolutionConflict)?;
        let taxonomy_categories = TaxonomyOwnerCategoryReader::new(self.db.clone())
            .load_scoped_categories(
                tenant_id,
                TaxonomyScopeType::Module,
                Some(FORUM_TAXONOMY_SCOPE),
                Some(&[taxonomy_category_id]),
                requested_locale.as_str(),
                fallback_locale.as_deref(),
            )
            .await
            .map_err(map_taxonomy_route_error)?;
        let taxonomy_category = match taxonomy_categories.as_slice() {
            [category] if category.id == taxonomy_category_id => category,
            _ => return Err(ForumError::CategoryRouteResolutionConflict),
        };
        if !taxonomy_category
            .available_locales
            .contains(&taxonomy_category.effective_locale)
        {
            return Err(ForumError::CategoryRouteResolutionConflict);
        }

        let locale = normalize_stored_locale(taxonomy_category.effective_locale.as_str())?;
        let slug = normalize_stored_slug(taxonomy_category.slug.as_str())?;
        Ok(ForumCategoryRouteDescriptor {
            category_id,
            path: forum_category_route_path(locale.as_str(), slug.as_str()),
            locale,
            slug,
        })
    }

    pub async fn resolve(
        &self,
        tenant_id: Uuid,
        requested_locale: &str,
        requested_slug: &str,
        fallback_locale: Option<&str>,
    ) -> ForumResult<ForumCategoryRouteResolution> {
        let requested_locale = normalize_route_locale(requested_locale)?;
        let requested_slug = normalize_route_slug(requested_slug)?;
        let fallback_locale = fallback_locale.map(normalize_route_locale).transpose()?;

        let route = TaxonomyService::new(self.db.clone())
            .resolve_term_route_for_module(
                tenant_id,
                TaxonomyTermKind::Category,
                FORUM_TAXONOMY_SCOPE,
                requested_locale.as_str(),
                fallback_locale.as_deref(),
                requested_slug.as_str(),
            )
            .await
            .map_err(map_taxonomy_route_error)?
            .ok_or(ForumError::CategoryRouteNotFound)?;
        if route.kind != TaxonomyTermKind::Category
            || route.scope_type != TaxonomyScopeType::Module
            || route.scope_value.as_deref() != Some(FORUM_TAXONOMY_SCOPE)
        {
            return Err(ForumError::CategoryRouteNotFound);
        }

        let binding = load_forum_binding_for_taxonomy(&self.db, tenant_id, route.term_id).await?;
        let category_id = binding
            .map(|binding| binding.forum_category_id)
            .ok_or(ForumError::CategoryRouteNotFound)?;
        ensure_active_category(&self.db, tenant_id, category_id).await?;

        let canonical = self
            .canonical_descriptor(
                tenant_id,
                category_id,
                requested_locale.as_str(),
                fallback_locale.as_deref(),
            )
            .await?;
        let disposition = if route.alias_id.is_none()
            && route.matched_locale == requested_locale
            && requested_locale == canonical.locale
            && requested_slug == canonical.slug
        {
            ForumCategoryRouteDisposition::Canonical
        } else {
            ForumCategoryRouteDisposition::Redirect
        };

        Ok(ForumCategoryRouteResolution {
            requested_locale,
            requested_slug,
            disposition,
            canonical,
            alias_id: route.alias_id,
        })
    }
}

async fn ensure_active_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let category_exists = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .is_some();
    if !category_exists {
        return Err(ForumError::CategoryRouteNotFound);
    }
    let archived = forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .filter(forum_category_lifecycle::Column::CategoryId.eq(category_id))
        .one(db)
        .await?
        .is_some();
    if archived {
        return Err(ForumError::CategoryRouteNotFound);
    }
    Ok(())
}

async fn load_taxonomy_binding_for_forum(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<Option<forum_category_taxonomy_binding::Model>> {
    forum_category_taxonomy_binding::Entity::find_by_id((tenant_id, category_id))
        .one(db)
        .await
        .map_err(ForumError::from)
}

async fn load_forum_binding_for_taxonomy(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    taxonomy_category_id: Uuid,
) -> ForumResult<Option<forum_category_taxonomy_binding::Model>> {
    forum_category_taxonomy_binding::Entity::find()
        .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .filter(
            forum_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(taxonomy_category_id),
        )
        .one(db)
        .await
        .map_err(ForumError::from)
}

fn map_taxonomy_route_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        _ => ForumError::CategoryRouteResolutionConflict,
    }
}

fn normalize_route_locale(value: &str) -> ForumResult<String> {
    let locale = normalize_locale_code(value)
        .ok_or_else(|| ForumError::Validation("Invalid forum category route locale".to_string()))?;
    if locale.chars().count() > MAX_FORUM_CATEGORY_ROUTE_LOCALE_LEN {
        return Err(ForumError::Validation(
            "Forum category route locale is too long".to_string(),
        ));
    }
    Ok(locale)
}

fn normalize_route_slug(value: &str) -> ForumResult<String> {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_dash = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    let normalized = normalized.trim_matches('-').to_string();
    if normalized.is_empty() || normalized.len() > MAX_FORUM_CATEGORY_ROUTE_SLUG_LEN {
        return Err(ForumError::CategoryRouteNotFound);
    }
    Ok(normalized)
}

fn normalize_stored_locale(value: &str) -> ForumResult<String> {
    normalize_route_locale(value).map_err(|_| ForumError::CategoryRouteResolutionConflict)
}

fn normalize_stored_slug(value: &str) -> ForumResult<String> {
    normalize_route_slug(value).map_err(|_| ForumError::CategoryRouteResolutionConflict)
}

fn forum_category_route_path(locale: &str, slug: &str) -> String {
    format!("/{locale}/forum/c/{slug}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_path_uses_normalized_locale_and_category_slug_policy() {
        let locale = normalize_route_locale(" EN_us ").expect("locale");
        let slug = normalize_route_slug(" A focused category! ").expect("slug");
        assert_eq!(
            forum_category_route_path(locale.as_str(), slug.as_str()),
            "/en-US/forum/c/a-focused-category"
        );
    }
}
