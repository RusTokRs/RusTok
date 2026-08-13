use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rustok_content::resolve_by_locale_with_fallback;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::{
    TaxonomyError, TaxonomyResult, TaxonomyScopeType, TaxonomyTermKind,
    entities::{taxonomy_term, taxonomy_term_translation},
    normalize_term_locale,
};

/// Localized Taxonomy vocabulary projection for a domain module that owns its
/// own term attachment tables and usage semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyOwnerTerm {
    pub id: Uuid,
    pub kind: TaxonomyTermKind,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: Option<String>,
    pub canonical_key: String,
    pub requested_locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

/// Storage-encapsulating read adapter for modules that own relations to Taxonomy terms.
///
/// Domain modules keep ownership of attachment tables and usage semantics. This reader
/// owns only Taxonomy scope filtering and localized vocabulary projection, so consumers
/// do not need to import Taxonomy persistence entities directly.
#[derive(Debug, Clone)]
pub struct TaxonomyOwnerReader {
    db: DatabaseConnection,
}

impl TaxonomyOwnerReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn load_scoped_terms(
        &self,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        scope_type: TaxonomyScopeType,
        scope_value: Option<&str>,
        term_ids: Option<&[Uuid]>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<Vec<TaxonomyOwnerTerm>> {
        if term_ids.is_some_and(|term_ids| term_ids.is_empty()) {
            return Ok(Vec::new());
        }

        let locale = normalize_term_locale(locale)
            .ok_or_else(|| TaxonomyError::validation("Locale cannot be empty"))?;
        let fallback_locale = fallback_locale
            .map(|fallback| {
                normalize_term_locale(fallback)
                    .ok_or_else(|| TaxonomyError::validation("Fallback locale cannot be empty"))
            })
            .transpose()?;
        let scope_value = normalize_scope(scope_type, scope_value)?;

        let mut query = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(kind))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(&scope_value));
        if let Some(term_ids) = term_ids {
            query = query.filter(taxonomy_term::Column::Id.is_in(term_ids.to_vec()));
        }

        let terms = query
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .all(&self.db)
            .await?;
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
        let translations = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids))
            .all(&self.db)
            .await?;
        let mut translations_by_term = HashMap::new();
        for translation in translations {
            translations_by_term
                .entry(translation.term_id)
                .or_insert_with(Vec::new)
                .push(translation);
        }

        Ok(terms
            .into_iter()
            .map(|term| {
                let translations = translations_by_term
                    .remove(&term.id)
                    .unwrap_or_default();
                let resolved = resolve_by_locale_with_fallback(
                    &translations,
                    &locale,
                    fallback_locale.as_deref(),
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

                TaxonomyOwnerTerm {
                    id: term.id,
                    kind: term.kind,
                    scope_type: term.scope_type,
                    scope_value: decode_scope(term.scope_type, &term.scope_value),
                    canonical_key: term.canonical_key,
                    requested_locale: locale.clone(),
                    effective_locale,
                    name,
                    slug,
                    created_at: term.created_at.into(),
                }
            })
            .collect())
    }
}

fn normalize_scope(scope_type: TaxonomyScopeType, scope_value: Option<&str>) -> TaxonomyResult<String> {
    match scope_type {
        TaxonomyScopeType::Global => {
            if scope_value.is_some_and(|scope_value| !scope_value.trim().is_empty()) {
                return Err(TaxonomyError::validation(
                    "Global taxonomy scope cannot have a scope value",
                ));
            }
            Ok(String::new())
        }
        TaxonomyScopeType::Module => {
            let scope_value = scope_value.map(str::trim).unwrap_or_default();
            if scope_value.is_empty() {
                return Err(TaxonomyError::validation(
                    "Module taxonomy scope requires a scope value",
                ));
            }
            if scope_value.chars().count() > 64 {
                return Err(TaxonomyError::validation(
                    "Taxonomy scope value cannot exceed 64 characters",
                ));
            }
            Ok(scope_value.to_owned())
        }
    }
}

fn decode_scope(scope_type: TaxonomyScopeType, scope_value: &str) -> Option<String> {
    match scope_type {
        TaxonomyScopeType::Global => None,
        TaxonomyScopeType::Module => Some(scope_value.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PLATFORM_FALLBACK_LOCALE;

    #[test]
    fn scope_normalization_preserves_taxonomy_storage_encoding() {
        assert_eq!(
            normalize_scope(TaxonomyScopeType::Global, None).expect("global scope"),
            ""
        );
        assert_eq!(
            normalize_scope(TaxonomyScopeType::Module, Some(" blog "))
                .expect("module scope"),
            "blog"
        );
        assert!(normalize_scope(TaxonomyScopeType::Module, None).is_err());
        assert!(normalize_scope(TaxonomyScopeType::Global, Some("blog")).is_err());
    }

    #[test]
    fn platform_locale_is_accepted_by_owner_reader_normalizer() {
        assert_eq!(
            normalize_term_locale(PLATFORM_FALLBACK_LOCALE),
            Some(PLATFORM_FALLBACK_LOCALE.to_owned())
        );
    }
}
