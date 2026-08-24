use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use rustok_content::resolve_by_locale_with_fallback;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use uuid::Uuid;

use crate::{
    TaxonomyCategoryMediaId, TaxonomyError, TaxonomyResult, TaxonomyScopeType, TaxonomyTermKind,
    entities::{
        taxonomy_category_hierarchy, taxonomy_category_presentation, taxonomy_term,
        taxonomy_term_translation,
    },
    normalize_term_locale,
};

/// Canonical Category projection for a domain module that owns a typed relation
/// to Taxonomy but must not read Taxonomy persistence entities directly.
///
/// The projection intentionally omits consumer policy/state such as Forum
/// moderation, counters and subscriptions. It contains only Taxonomy-owned
/// identity, localized copy, hierarchy and canonical presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyOwnerCategory {
    pub id: Uuid,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: Option<String>,
    pub canonical_key: String,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub image_media_id: Option<TaxonomyCategoryMediaId>,
    pub cover_media_id: Option<TaxonomyCategoryMediaId>,
    pub presentation_revision: i64,
    pub created_at: DateTime<Utc>,
}

/// Storage-encapsulating Category read adapter for consumer-owned typed bindings.
///
/// Consumer modules authorize their own API resource before calling this reader.
/// Taxonomy remains responsible for tenant/scope/Category identity boundaries and
/// for composing the canonical localized/hierarchy/presentation snapshot. No
/// `Resource::Taxonomy` permission is required merely to follow an already-owned
/// consumer binding.
#[derive(Debug, Clone)]
pub struct TaxonomyOwnerCategoryReader {
    db: DatabaseConnection,
}

impl TaxonomyOwnerCategoryReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Loads Category snapshots in one bounded owner read.
    ///
    /// `term_ids=None` lists the selected Category scope. `Some(ids)` restricts
    /// the projection to those identities, while `Some(&[])` is an explicit
    /// empty page and never widens into a scope-wide read.
    pub async fn load_scoped_categories(
        &self,
        tenant_id: Uuid,
        scope_type: TaxonomyScopeType,
        scope_value: Option<&str>,
        term_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<Vec<TaxonomyOwnerCategory>> {
        if term_ids.is_some_and(|term_ids| term_ids.is_empty()) {
            return Ok(Vec::new());
        }

        let locale = normalize_requested_locale(locale)?;
        let fallback_locale = normalize_fallback_locale(fallback_locale)?;
        let scope_value = normalize_scope_value(scope_type, scope_value)?;

        let mut query = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(&scope_value));
        if let Some(term_ids) = term_ids {
            query = query.filter(taxonomy_term::Column::Id.is_in(term_ids.to_vec()));
        }

        let terms = query
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .all(&self.db)
            .await?;

        materialize_categories(
            &self.db,
            tenant_id,
            terms,
            &locale,
            fallback_locale.as_deref(),
        )
        .await
    }
}

async fn materialize_categories<C>(
    connection: &C,
    tenant_id: Uuid,
    terms: Vec<taxonomy_term::Model>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> TaxonomyResult<Vec<TaxonomyOwnerCategory>>
where
    C: ConnectionTrait,
{
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids.clone()))
        .all(connection)
        .await?;
    let hierarchy = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.is_in(term_ids.clone()))
        .all(connection)
        .await?;
    let presentation = taxonomy_category_presentation::Entity::find()
        .filter(taxonomy_category_presentation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_presentation::Column::TermId.is_in(term_ids))
        .all(connection)
        .await?;

    let mut translations_by_term = HashMap::<Uuid, Vec<taxonomy_term_translation::Model>>::new();
    for translation in translations {
        translations_by_term
            .entry(translation.term_id)
            .or_default()
            .push(translation);
    }
    let mut hierarchy_by_term = hierarchy
        .into_iter()
        .map(|row| (row.term_id, row))
        .collect::<HashMap<_, _>>();
    let mut presentation_by_term = presentation
        .into_iter()
        .map(|row| (row.term_id, row))
        .collect::<HashMap<_, _>>();

    let mut categories = Vec::with_capacity(terms.len());
    for term in terms {
        let translations = translations_by_term.remove(&term.id).unwrap_or_default();
        let available_locales = collect_available_locales(term.id, &translations)?;
        let resolved = resolve_by_locale_with_fallback(
            &translations,
            locale,
            fallback_locale,
            |translation| translation.locale.as_str(),
        );
        let effective_locale = resolved.effective_locale;
        let name = resolved
            .item
            .map(|translation| translation.name.clone())
            .unwrap_or_else(|| term.canonical_key.clone());
        let slug = resolved
            .item
            .map(|translation| translation.slug.clone())
            .unwrap_or_else(|| term.canonical_key.clone());
        let description = resolved
            .item
            .and_then(|translation| translation.description.clone());

        let (parent_id, position) = hierarchy_by_term
            .remove(&term.id)
            .map(|row| (row.parent_term_id, row.position))
            .unwrap_or((None, 0));
        let (icon_key, color, image_media_id, cover_media_id, presentation_revision) =
            presentation_by_term
                .remove(&term.id)
                .map(|row| {
                    (
                        row.icon_key,
                        row.color,
                        row.image_media_id.map(TaxonomyCategoryMediaId::from),
                        row.cover_media_id.map(TaxonomyCategoryMediaId::from),
                        row.revision,
                    )
                })
                .unwrap_or((None, None, None, None, 0));

        categories.push(TaxonomyOwnerCategory {
            id: term.id,
            scope_type: term.scope_type,
            scope_value: decode_scope_value(term.scope_type, &term.scope_value),
            canonical_key: term.canonical_key,
            requested_locale: locale.to_owned(),
            effective_locale,
            available_locales,
            name,
            slug,
            description,
            parent_id,
            position,
            icon_key,
            color,
            image_media_id,
            cover_media_id,
            presentation_revision,
            created_at: term.created_at.into(),
        });
    }

    Ok(categories)
}

fn collect_available_locales(
    term_id: Uuid,
    translations: &[taxonomy_term_translation::Model],
) -> TaxonomyResult<Vec<String>> {
    let mut locales = BTreeSet::new();
    for translation in translations {
        let locale = normalize_term_locale(&translation.locale).ok_or_else(|| {
            TaxonomyError::validation(format!(
                "Taxonomy Category {term_id} has an invalid persisted locale {:?}",
                translation.locale
            ))
        })?;
        locales.insert(locale);
    }
    Ok(locales.into_iter().collect())
}

fn normalize_requested_locale(locale: &str) -> TaxonomyResult<String> {
    normalize_term_locale(locale).ok_or_else(|| TaxonomyError::validation("Locale cannot be empty"))
}

fn normalize_fallback_locale(fallback_locale: Option<&str>) -> TaxonomyResult<Option<String>> {
    fallback_locale
        .map(|fallback| {
            normalize_term_locale(fallback)
                .ok_or_else(|| TaxonomyError::validation("Fallback locale cannot be empty"))
        })
        .transpose()
}

fn normalize_scope_value(
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
) -> TaxonomyResult<String> {
    match scope_type {
        TaxonomyScopeType::Global => Ok(String::new()),
        TaxonomyScopeType::Module => {
            let value = scope_value
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
                .collect::<String>();
            if value.is_empty() {
                return Err(TaxonomyError::validation(
                    "Module scope requires a non-empty scope_value",
                ));
            }
            Ok(value)
        }
    }
}

fn decode_scope_value(scope_type: TaxonomyScopeType, scope_value: &str) -> Option<String> {
    match scope_type {
        TaxonomyScopeType::Global => None,
        TaxonomyScopeType::Module => Some(scope_value.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_owner_scope_normalization_matches_taxonomy_storage() {
        assert_eq!(
            normalize_scope_value(TaxonomyScopeType::Module, Some(" Forum! "))
                .expect("module scope"),
            "forum"
        );
        assert_eq!(
            normalize_scope_value(TaxonomyScopeType::Global, Some("ignored"))
                .expect("global scope"),
            ""
        );
        assert!(normalize_scope_value(TaxonomyScopeType::Module, None).is_err());
    }
}
