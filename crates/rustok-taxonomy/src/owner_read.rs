use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rustok_content::resolve_by_locale_with_fallback;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder,
};
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
/// owns only Taxonomy identity, scope filtering and localized vocabulary projection, so
/// consumers do not need to import Taxonomy persistence entities directly.
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

        let locale = normalize_requested_locale(locale)?;
        let fallback_locale = normalize_fallback_locale(fallback_locale)?;
        let scope_value = normalize_scope_value(scope_type, scope_value)?;

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
        materialize_terms(
            &self.db,
            tenant_id,
            terms,
            &locale,
            fallback_locale.as_deref(),
        )
        .await
    }

    /// Loads attached Taxonomy terms without leaving the caller's transaction.
    ///
    /// This is intentionally ID-oriented rather than scope-oriented: owner relation
    /// tables may legitimately reference both module-local and global terms. Tenant
    /// and kind remain mandatory identity boundaries and are enforced by Taxonomy.
    pub async fn load_terms_by_ids_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        kind: TaxonomyTermKind,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<Vec<TaxonomyOwnerTerm>> {
        if term_ids.is_empty() {
            return Ok(Vec::new());
        }

        let locale = normalize_requested_locale(locale)?;
        let fallback_locale = normalize_fallback_locale(fallback_locale)?;
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(kind))
            .filter(taxonomy_term::Column::Id.is_in(term_ids.to_vec()))
            .order_by_asc(taxonomy_term::Column::CanonicalKey)
            .all(txn)
            .await?;

        materialize_terms(txn, tenant_id, terms, &locale, fallback_locale.as_deref()).await
    }
}

async fn materialize_terms<C>(
    connection: &C,
    tenant_id: Uuid,
    terms: Vec<taxonomy_term::Model>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> TaxonomyResult<Vec<TaxonomyOwnerTerm>>
where
    C: ConnectionTrait,
{
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let term_ids = terms.iter().map(|term| term.id).collect::<Vec<_>>();
    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids))
        .all(connection)
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
            let translations = translations_by_term.remove(&term.id).unwrap_or_default();
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

            TaxonomyOwnerTerm {
                id: term.id,
                kind: term.kind,
                scope_type: term.scope_type,
                scope_value: decode_scope_value(term.scope_type, &term.scope_value),
                canonical_key: term.canonical_key,
                requested_locale: locale.to_owned(),
                effective_locale,
                name,
                slug,
                created_at: term.created_at.into(),
            }
        })
        .collect())
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
            let value = normalize_optional_scope_label(scope_value.unwrap_or_default());
            if value.is_empty() {
                return Err(TaxonomyError::validation(
                    "Module scope requires a non-empty scope_value",
                ));
            }
            Ok(value)
        }
    }
}

fn normalize_optional_scope_label(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect()
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
    use rustok_api::PLATFORM_FALLBACK_LOCALE;

    #[test]
    fn scope_normalization_matches_taxonomy_storage_encoding() {
        assert_eq!(
            normalize_scope_value(TaxonomyScopeType::Global, Some("ignored"))
                .expect("global scope"),
            ""
        );
        assert_eq!(
            normalize_scope_value(TaxonomyScopeType::Module, Some(" Blog! "))
                .expect("module scope"),
            "blog"
        );
        assert!(normalize_scope_value(TaxonomyScopeType::Module, None).is_err());
    }

    #[test]
    fn platform_locale_is_accepted_by_owner_reader_normalizer() {
        assert_eq!(
            normalize_term_locale(PLATFORM_FALLBACK_LOCALE),
            Some(PLATFORM_FALLBACK_LOCALE.to_owned())
        );
    }
}
