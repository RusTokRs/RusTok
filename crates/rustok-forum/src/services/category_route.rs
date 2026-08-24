use std::collections::HashSet;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_content::normalize_locale_code;
use rustok_taxonomy::{
    TaxonomyOwnerCategoryReader, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_lifecycle, forum_category_taxonomy_binding,
    forum_category_translation,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct CategoryRouteCandidate {
    category_id: Uuid,
    locale: String,
    active: bool,
    alias_id: Option<Uuid>,
}

/// Forum route facade for Taxonomy-owned Category route identity.
///
/// Canonical localized slugs and immutable aliases are resolved through the
/// Taxonomy route-key registry. Forum retains ownership of route disclosure:
/// the resolved Taxonomy Category must map through the same-tenant typed Forum
/// binding and the Forum category must remain active. The public path remains
/// `/{locale}/forum/c/{slug}` and hierarchy is intentionally absent from it.
///
/// Legacy Forum route rows are retained temporarily for command-side collision
/// validation during CAT-5, but public route reads do not fall back to them.
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

async fn load_category_translations(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<Vec<forum_category_translation::Model>> {
    let translations = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .order_by_asc(forum_category_translation::Column::Locale)
        .order_by_asc(forum_category_translation::Column::Id)
        .limit(MAX_FORUM_CATEGORY_ROUTE_CANDIDATES + 1)
        .all(db)
        .await?;
    if translations.len() > MAX_FORUM_CATEGORY_ROUTE_CANDIDATES as usize {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    Ok(translations)
}

async fn load_route_candidates(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
) -> ForumResult<Vec<CategoryRouteCandidate>> {
    let translations = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::Slug.eq(slug))
        .order_by_asc(forum_category_translation::Column::Locale)
        .order_by_asc(forum_category_translation::Column::CategoryId)
        .limit(MAX_FORUM_CATEGORY_ROUTE_CANDIDATES + 1)
        .all(db)
        .await?;
    if translations.len() > MAX_FORUM_CATEGORY_ROUTE_CANDIDATES as usize {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }

    let mut candidates = Vec::with_capacity(translations.len());
    if !translations.is_empty() {
        let category_ids = translations
            .iter()
            .map(|translation| translation.category_id)
            .collect::<HashSet<_>>();
        let (existing_ids, archived_ids) =
            load_category_route_state(db, tenant_id, &category_ids).await?;
        if existing_ids != category_ids {
            return Err(ForumError::CategoryRouteResolutionConflict);
        }

        for translation in translations {
            let locale = normalize_stored_locale(translation.locale.as_str())?;
            let stored_slug = normalize_stored_slug(translation.slug.as_str())?;
            if stored_slug != slug {
                return Err(ForumError::CategoryRouteResolutionConflict);
            }
            candidates.push(CategoryRouteCandidate {
                category_id: translation.category_id,
                locale,
                active: !archived_ids.contains(&translation.category_id),
                alias_id: None,
            });
        }
    }

    candidates.extend(load_alias_route_candidates(db, tenant_id, slug).await?);
    if candidates.len() > MAX_FORUM_CATEGORY_ROUTE_CANDIDATES as usize {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    if candidates.is_empty() {
        return Err(ForumError::CategoryRouteNotFound);
    }
    Ok(candidates)
}

async fn load_category_route_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_ids: &HashSet<Uuid>,
) -> ForumResult<(HashSet<Uuid>, HashSet<Uuid>)> {
    if category_ids.is_empty() {
        return Ok((HashSet::new(), HashSet::new()));
    }
    let category_id_list = category_ids.iter().copied().collect::<Vec<_>>();
    let existing_ids = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .filter(forum_category::Column::Id.is_in(category_id_list.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|category| category.id)
        .collect::<HashSet<_>>();
    let archived_ids = forum_category_lifecycle::Entity::find()
        .filter(forum_category_lifecycle::Column::TenantId.eq(tenant_id))
        .filter(forum_category_lifecycle::Column::CategoryId.is_in(category_id_list))
        .all(db)
        .await?
        .into_iter()
        .map(|lifecycle| lifecycle.category_id)
        .collect::<HashSet<_>>();
    Ok((existing_ids, archived_ids))
}

fn select_route_candidate<'a>(
    candidates: &'a [CategoryRouteCandidate],
    requested_locale: &str,
    fallback_locale: Option<&str>,
) -> ForumResult<&'a CategoryRouteCandidate> {
    let mut preferred_locales = Vec::<&str>::with_capacity(3);
    for locale in [
        Some(requested_locale),
        fallback_locale,
        Some(PLATFORM_FALLBACK_LOCALE),
    ]
    .into_iter()
    .flatten()
    {
        if !preferred_locales.contains(&locale) {
            preferred_locales.push(locale);
        }
    }

    for locale in preferred_locales {
        if let Some(candidate) = unique_candidate_for_locale(candidates, locale)? {
            return if candidate.active {
                Ok(candidate)
            } else {
                Err(ForumError::CategoryRouteNotFound)
            };
        }
    }

    let active_candidates = candidates
        .iter()
        .filter(|candidate| candidate.active)
        .collect::<Vec<_>>();
    if active_candidates.is_empty() {
        return Err(ForumError::CategoryRouteNotFound);
    }
    let category_ids = active_candidates
        .iter()
        .map(|candidate| candidate.category_id)
        .collect::<HashSet<_>>();
    if category_ids.len() != 1 {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    active_candidates
        .first()
        .copied()
        .ok_or(ForumError::CategoryRouteNotFound)
}

fn unique_candidate_for_locale<'a>(
    candidates: &'a [CategoryRouteCandidate],
    locale: &str,
) -> ForumResult<Option<&'a CategoryRouteCandidate>> {
    let mut matches = candidates
        .iter()
        .filter(|candidate| candidate.locale == locale);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    Ok(first)
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

fn normalize_route_slug_for_write(value: &str) -> ForumResult<String> {
    normalize_route_slug(value).map_err(|error| match error {
        ForumError::CategoryRouteNotFound => ForumError::Validation(
            "Forum category route slug must contain a valid route segment".to_string(),
        ),
        other => other,
    })
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

include!("category_route_alias.rs");

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(category_id: Uuid, locale: &str, active: bool) -> CategoryRouteCandidate {
        CategoryRouteCandidate {
            category_id,
            locale: locale.to_string(),
            active,
            alias_id: None,
        }
    }

    #[test]
    fn canonical_path_uses_normalized_locale_and_category_slug_policy() {
        let locale = normalize_route_locale(" EN_us ").expect("locale");
        let slug = normalize_route_slug(" A focused category! ").expect("slug");
        assert_eq!(
            forum_category_route_path(locale.as_str(), slug.as_str()),
            "/en-US/forum/c/a-focused-category"
        );
    }

    #[test]
    fn exact_locale_precedes_fallback_and_archived_exact_does_not_fall_through() {
        let exact_id = Uuid::new_v4();
        let fallback_id = Uuid::new_v4();
        let candidates = [
            candidate(fallback_id, "en", true),
            candidate(exact_id, "fr", true),
        ];
        assert_eq!(
            select_route_candidate(&candidates, "fr", Some("en"))
                .expect("exact")
                .category_id,
            exact_id
        );

        let archived = [
            candidate(fallback_id, "en", true),
            candidate(exact_id, "fr", false),
        ];
        assert!(matches!(
            select_route_candidate(&archived, "fr", Some("en")),
            Err(ForumError::CategoryRouteNotFound)
        ));
    }

    #[test]
    fn exact_alias_precedes_fallback_current_route() {
        let alias_id = Uuid::new_v4();
        let exact_category_id = Uuid::new_v4();
        let fallback_category_id = Uuid::new_v4();
        let candidates = [
            CategoryRouteCandidate {
                category_id: fallback_category_id,
                locale: "en".to_string(),
                active: true,
                alias_id: None,
            },
            CategoryRouteCandidate {
                category_id: exact_category_id,
                locale: "fr".to_string(),
                active: true,
                alias_id: Some(alias_id),
            },
        ];
        let selected = select_route_candidate(&candidates, "fr", Some("en")).expect("exact");
        assert_eq!(selected.category_id, exact_category_id);
        assert_eq!(selected.alias_id, Some(alias_id));
    }

    #[test]
    fn residual_first_available_requires_one_category_identity() {
        let category_id = Uuid::new_v4();
        let one_category = [
            candidate(category_id, "de", true),
            candidate(category_id, "it", true),
        ];
        assert_eq!(
            select_route_candidate(&one_category, "fr", None)
                .expect("one category")
                .locale,
            "de"
        );

        let ambiguous = [
            candidate(Uuid::new_v4(), "de", true),
            candidate(Uuid::new_v4(), "it", true),
        ];
        assert!(matches!(
            select_route_candidate(&ambiguous, "fr", None),
            Err(ForumError::CategoryRouteResolutionConflict)
        ));
    }
}
