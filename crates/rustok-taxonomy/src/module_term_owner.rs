use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use uuid::Uuid;

use crate::dto::{TaxonomyScopeType, TaxonomyTermKind, TaxonomyTermStatus};
use crate::entities::{taxonomy_term, taxonomy_term_translation};
use crate::error::{TaxonomyError, TaxonomyResult};
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

/// Bounded owner-side create input for a module-scoped taxonomy term.
///
/// The caller supplies the identity so a domain module can migrate an existing
/// stable resource id into Taxonomy without introducing an id translation
/// table. This is an internal module-composition API, not an end-user Taxonomy
/// CRUD bypass; the consuming module remains responsible for its own command
/// authorization and domain transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOwnedTermCreateInput {
    pub id: Uuid,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOwnedTermUpdateInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOwnedTermTranslation {
    pub id: Uuid,
    pub term_id: Uuid,
    pub tenant_id: Uuid,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOwnedTermResolved {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub status: TaxonomyTermStatus,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleOwnedTermUpdateResult {
    pub previous_slug: Option<String>,
    pub current: ModuleOwnedTermResolved,
}

/// Owner boundary for one `(kind, module_slug)` taxonomy namespace.
///
/// Domain modules use this type instead of reading `taxonomy_*` entities. The
/// API deliberately supports caller-owned transactions so a module can commit
/// its extension state and the Taxonomy-owned term atomically while Taxonomy
/// remains the only code that reads or mutates its persistence tables.
pub struct ModuleTermOwnerService {
    db: DatabaseConnection,
    kind: TaxonomyTermKind,
    module_scope: String,
}

impl ModuleTermOwnerService {
    pub fn new(
        db: DatabaseConnection,
        kind: TaxonomyTermKind,
        module_slug: &str,
    ) -> TaxonomyResult<Self> {
        Ok(Self {
            db,
            kind,
            module_scope: normalize_module_scope(module_slug)?,
        })
    }

    pub async fn create_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        input: ModuleOwnedTermCreateInput,
    ) -> TaxonomyResult<ModuleOwnedTermResolved> {
        if tenant_id.is_nil() || input.id.is_nil() {
            return Err(TaxonomyError::validation(
                "module-owned taxonomy terms require non-nil tenant and term ids",
            ));
        }
        let locale = normalize_locale(&input.locale)?;
        validate_term_name(&input.name)?;
        validate_optional_description(input.description.as_deref())?;
        let slug = normalize_non_empty_slug(&input.slug)?;

        if taxonomy_term::Entity::find_by_id(input.id)
            .one(txn)
            .await?
            .is_some()
        {
            return Err(TaxonomyError::conflict(format!(
                "taxonomy term {} already exists",
                input.id
            )));
        }
        self.ensure_slug_available_in_tx(txn, tenant_id, &locale, &slug, None)
            .await?;

        let now = input.created_at.unwrap_or_else(Utc::now);
        let canonical_key = format!("{}-{}", self.module_scope, input.id.simple());
        let created_term = taxonomy_term::ActiveModel {
            id: Set(input.id),
            tenant_id: Set(tenant_id),
            kind: Set(self.kind),
            scope_type: Set(TaxonomyScopeType::Module),
            scope_value: Set(self.module_scope.clone()),
            canonical_key: Set(canonical_key),
            status: Set(TaxonomyTermStatus::Active),
            revision: Set(1),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(txn)
        .await?;
        let created_translation = taxonomy_term_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            term_id: Set(input.id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(input.name.clone()),
            slug: Set(slug.clone()),
            description: Set(input.description.clone()),
            revision: Set(1),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        }
        .insert(txn)
        .await?;
        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                term_id: input.id,
                locale: &locale,
                resource_revision: created_term.revision,
                target_revision: created_translation.revision,
                operation: "upsert",
                lifecycle: "active",
            },
        )
        .await?;

        Ok(ModuleOwnedTermResolved {
            id: input.id,
            tenant_id,
            requested_locale: locale.clone(),
            effective_locale: locale.clone(),
            available_locales: vec![locale],
            name: input.name,
            slug,
            description: input.description,
            status: TaxonomyTermStatus::Active,
            revision: created_term.revision,
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn update_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
        input: ModuleOwnedTermUpdateInput,
    ) -> TaxonomyResult<ModuleOwnedTermUpdateResult> {
        let locale = normalize_locale(&input.locale)?;
        let term = self.find_term_in_tx(txn, tenant_id, term_id).await?;
        let now = Utc::now();
        let existing = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
            .one(txn)
            .await?;
        let previous_slug = existing.as_ref().map(|value| value.slug.clone());

        let target_revision = match existing {
            Some(existing) => {
                let name = input.name.clone().unwrap_or_else(|| existing.name.clone());
                validate_term_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug) => normalize_non_empty_slug(slug)?,
                    None if input.name.is_some() => normalize_non_empty_slug(&name)?,
                    None => existing.slug.clone(),
                };
                self.ensure_slug_available_in_tx(txn, tenant_id, &locale, &slug, Some(term_id))
                    .await?;
                let description = input
                    .description
                    .clone()
                    .or_else(|| existing.description.clone());
                validate_optional_description(description.as_deref())?;
                let revision = next_translation_revision(term_id, &locale, existing.revision)?;
                let mut active: taxonomy_term_translation::ActiveModel = existing.into();
                active.name = Set(name);
                active.slug = Set(slug);
                active.description = Set(description);
                active.revision = Set(revision);
                active.updated_at = Set(now.fixed_offset());
                active.update(txn).await?;
                revision
            }
            None => {
                let name = input.name.clone().ok_or_else(|| {
                    TaxonomyError::validation("Name is required when adding a new locale")
                })?;
                validate_term_name(&name)?;
                let slug = normalize_non_empty_slug(input.slug.as_deref().unwrap_or(&name))?;
                self.ensure_slug_available_in_tx(txn, tenant_id, &locale, &slug, Some(term_id))
                    .await?;
                validate_optional_description(input.description.as_deref())?;
                taxonomy_term_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    term_id: Set(term_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name),
                    slug: Set(slug),
                    description: Set(input.description.clone()),
                    revision: Set(1),
                    created_at: Set(now.fixed_offset()),
                    updated_at: Set(now.fixed_offset()),
                }
                .insert(txn)
                .await?;
                1
            }
        };

        let resource_revision = next_term_revision(&term)?;
        let mut active: taxonomy_term::ActiveModel = term.clone().into();
        active.revision = Set(resource_revision);
        active.updated_at = Set(now.fixed_offset());
        active.update(txn).await?;
        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                term_id,
                locale: &locale,
                resource_revision,
                target_revision,
                operation: "upsert",
                lifecycle: lifecycle_for_status(term.status),
            },
        )
        .await?;

        let current = self
            .resolve_term_in_tx(txn, tenant_id, term_id, &locale, None)
            .await?;
        Ok(ModuleOwnedTermUpdateResult {
            previous_slug,
            current,
        })
    }

    pub async fn delete_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
    ) -> TaxonomyResult<()> {
        let term = self.find_term_in_tx(txn, tenant_id, term_id).await?;
        let deleted = taxonomy_term::Entity::delete_many()
            .filter(taxonomy_term::Column::Id.eq(term_id))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .filter(taxonomy_term::Column::Revision.eq(term.revision))
            .exec(txn)
            .await?;
        if deleted.rows_affected != 1 {
            return Err(TaxonomyError::conflict(
                "taxonomy term changed before module-owned deletion could commit",
            ));
        }
        Ok(())
    }

    pub async fn resolve_term(
        &self,
        tenant_id: Uuid,
        term_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<ModuleOwnedTermResolved> {
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let term = self.find_term(tenant_id, term_id).await?;
        let translations = self.translations_for_terms(tenant_id, &[term_id]).await?;
        resolve_term_record(
            term,
            translations.get(&term_id).cloned().unwrap_or_default(),
            &locale,
            fallback_locale.as_deref(),
        )
    }

    pub async fn resolve_terms(
        &self,
        tenant_id: Uuid,
        term_ids: &[Uuid],
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<HashMap<Uuid, ModuleOwnedTermResolved>> {
        if term_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let locale = normalize_locale(locale)?;
        let fallback_locale = fallback_locale.map(normalize_locale).transpose()?;
        let terms = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .filter(taxonomy_term::Column::Id.is_in(term_ids.to_vec()))
            .all(&self.db)
            .await?;
        let translations = self.translations_for_terms(tenant_id, term_ids).await?;
        let mut result = HashMap::with_capacity(terms.len());
        for term in terms {
            let id = term.id;
            let resolved = resolve_term_record(
                term,
                translations.get(&id).cloned().unwrap_or_default(),
                &locale,
                fallback_locale.as_deref(),
            )?;
            result.insert(id, resolved);
        }
        Ok(result)
    }

    pub async fn translations_for_terms(
        &self,
        tenant_id: Uuid,
        term_ids: &[Uuid],
    ) -> TaxonomyResult<HashMap<Uuid, Vec<ModuleOwnedTermTranslation>>> {
        if term_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::TermId.is_in(term_ids.to_vec()))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .order_by_asc(taxonomy_term_translation::Column::TermId)
            .order_by_asc(taxonomy_term_translation::Column::Locale)
            .all(&self.db)
            .await?;
        let mut result = HashMap::new();
        for row in rows {
            result
                .entry(row.term_id)
                .or_insert_with(Vec::new)
                .push(map_translation(row));
        }
        Ok(result)
    }

    pub async fn translations_matching_slug(
        &self,
        tenant_id: Uuid,
        slug: &str,
        limit: u64,
    ) -> TaxonomyResult<Vec<ModuleOwnedTermTranslation>> {
        let slug = normalize_non_empty_slug(slug)?;
        let rows = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .order_by_asc(taxonomy_term_translation::Column::Locale)
            .order_by_asc(taxonomy_term_translation::Column::TermId)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(map_translation).collect())
    }

    pub async fn ensure_exists_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
    ) -> TaxonomyResult<()> {
        self.find_term_in_tx(txn, tenant_id, term_id).await?;
        Ok(())
    }

    async fn resolve_term_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> TaxonomyResult<ModuleOwnedTermResolved> {
        let term = self.find_term_in_tx(txn, tenant_id, term_id).await?;
        let translations = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .order_by_asc(taxonomy_term_translation::Column::Locale)
            .all(txn)
            .await?
            .into_iter()
            .map(map_translation)
            .collect();
        resolve_term_record(term, translations, locale, fallback_locale)
    }

    async fn find_term(
        &self,
        tenant_id: Uuid,
        term_id: Uuid,
    ) -> TaxonomyResult<taxonomy_term::Model> {
        taxonomy_term::Entity::find_by_id(term_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .one(&self.db)
            .await?
            .ok_or(TaxonomyError::TermNotFound(term_id))
    }

    async fn find_term_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        term_id: Uuid,
    ) -> TaxonomyResult<taxonomy_term::Model> {
        taxonomy_term::Entity::find_by_id(term_id)
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope))
            .one(txn)
            .await?
            .ok_or(TaxonomyError::TermNotFound(term_id))
    }

    async fn ensure_slug_available_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        locale: &str,
        slug: &str,
        exclude_term_id: Option<Uuid>,
    ) -> TaxonomyResult<()> {
        let mut select = taxonomy_term_translation::Entity::find()
            .join(
                JoinType::InnerJoin,
                taxonomy_term_translation::Relation::Term.def(),
            )
            .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_translation::Column::Locale.eq(locale))
            .filter(taxonomy_term_translation::Column::Slug.eq(slug))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(self.kind))
            .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
            .filter(taxonomy_term::Column::ScopeValue.eq(&self.module_scope));
        if let Some(exclude_term_id) = exclude_term_id {
            select = select.filter(taxonomy_term_translation::Column::TermId.ne(exclude_term_id));
        }
        if select.one(txn).await?.is_some() {
            return Err(TaxonomyError::DuplicateSlug(slug.to_string()));
        }
        Ok(())
    }
}

fn resolve_term_record(
    term: taxonomy_term::Model,
    translations: Vec<ModuleOwnedTermTranslation>,
    locale: &str,
    fallback_locale: Option<&str>,
) -> TaxonomyResult<ModuleOwnedTermResolved> {
    let resolved = resolve_by_locale_with_fallback(
        &translations,
        locale,
        fallback_locale,
        |translation| translation.locale.as_str(),
    );
    let translation = resolved.item.ok_or_else(|| {
        TaxonomyError::validation(format!("taxonomy term {} has no localized translation", term.id))
    })?;
    Ok(ModuleOwnedTermResolved {
        id: term.id,
        tenant_id: term.tenant_id,
        requested_locale: locale.to_string(),
        effective_locale: resolved.effective_locale,
        available_locales: available_locales_from(&translations, |translation| {
            translation.locale.as_str()
        }),
        name: translation.name.clone(),
        slug: translation.slug.clone(),
        description: translation.description.clone(),
        status: term.status,
        revision: term.revision,
        created_at: term.created_at.with_timezone(&Utc),
        updated_at: term.updated_at.with_timezone(&Utc),
    })
}

fn map_translation(row: taxonomy_term_translation::Model) -> ModuleOwnedTermTranslation {
    ModuleOwnedTermTranslation {
        id: row.id,
        term_id: row.term_id,
        tenant_id: row.tenant_id,
        locale: row.locale,
        name: row.name,
        slug: row.slug,
        description: row.description,
        revision: row.revision,
    }
}

fn normalize_module_scope(module_slug: &str) -> TaxonomyResult<String> {
    let value = module_slug
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

fn normalize_locale(locale: &str) -> TaxonomyResult<String> {
    normalize_locale_code(locale).ok_or_else(|| TaxonomyError::validation("Invalid locale"))
}

fn normalize_non_empty_slug(value: &str) -> TaxonomyResult<String> {
    let slug = slug::slugify(value);
    if slug.is_empty() || slug.len() > 120 {
        return Err(TaxonomyError::validation(
            "Localized slug must contain a valid value up to 120 bytes",
        ));
    }
    Ok(slug)
}

fn validate_term_name(name: &str) -> TaxonomyResult<()> {
    if name.trim().is_empty() || name.chars().count() > 120 {
        return Err(TaxonomyError::validation(
            "Term name must contain between 1 and 120 characters",
        ));
    }
    Ok(())
}

fn validate_optional_description(description: Option<&str>) -> TaxonomyResult<()> {
    if description.is_some_and(|value| value.chars().count() > 4_000) {
        return Err(TaxonomyError::validation(
            "Term description cannot exceed 4000 characters",
        ));
    }
    Ok(())
}

fn next_term_revision(term: &taxonomy_term::Model) -> TaxonomyResult<i64> {
    term.revision
        .checked_add(1)
        .filter(|revision| term.revision > 0 && *revision > 0)
        .ok_or_else(|| {
            TaxonomyError::conflict(format!(
                "taxonomy term {} has an invalid or exhausted resource revision",
                term.id
            ))
        })
}

fn next_translation_revision(term_id: Uuid, locale: &str, revision: i64) -> TaxonomyResult<i64> {
    revision
        .checked_add(1)
        .filter(|next_revision| revision > 0 && *next_revision > 0)
        .ok_or_else(|| TaxonomyError::TranslationRevisionExhausted {
            term_id,
            locale: locale.to_string(),
        })
}

fn lifecycle_for_status(status: TaxonomyTermStatus) -> &'static str {
    match status {
        TaxonomyTermStatus::Active => "active",
        TaxonomyTermStatus::Deprecated => "archived",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_scope_and_slug_normalization_are_bounded() {
        assert_eq!(normalize_module_scope(" Forum ").unwrap(), "forum");
        assert_eq!(normalize_non_empty_slug(" General Discussion ").unwrap(), "general-discussion");
        assert!(normalize_module_scope("***").is_err());
        assert!(normalize_non_empty_slug("***").is_err());
    }
}
