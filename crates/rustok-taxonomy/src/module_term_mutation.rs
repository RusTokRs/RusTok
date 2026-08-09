use chrono::{DateTime, Utc};
use rustok_api::{Action, Resource};
use rustok_content::normalize_locale_code;
use rustok_core::{PermissionScope, SecurityContext};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, JoinType,
    QueryFilter, RelationTrait, sea_query::Expr,
};
use uuid::Uuid;

use crate::dto::{TaxonomyScopeType, TaxonomyTermKind, TaxonomyTermStatus};
use crate::entities::{taxonomy_term, taxonomy_term_translation};
use crate::error::{TaxonomyError, TaxonomyResult};
use crate::translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleTermUpdateInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleTermMutationResult {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub locale: String,
    pub effective_locale: String,
    pub name: String,
    pub slug: String,
    pub created_at: DateTime<Utc>,
}

pub async fn update_module_term_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    security: &SecurityContext,
    kind: TaxonomyTermKind,
    module_slug: &str,
    input: ModuleTermUpdateInput,
) -> TaxonomyResult<ModuleTermMutationResult> {
    // The pre-existing public update path requires Update and then Read when it
    // materializes the response. Preserve that successful-call permission set.
    enforce_scope(security, Resource::Taxonomy, Action::Update)?;
    enforce_scope(security, Resource::Taxonomy, Action::Read)?;

    let module_scope = normalize_module_scope(module_slug)?;
    let locale = normalize_locale(&input.locale)?;
    let term = find_module_term_in_tx(txn, tenant_id, term_id, kind, &module_scope).await?;
    let now = Utc::now();

    let existing_translation = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
        .one(txn)
        .await?;

    let (translation_id, name, slug, target_revision, created_at) =
        match existing_translation {
            Some(existing) => {
                let name = input.name.clone().unwrap_or_else(|| existing.name.clone());
                validate_term_name(&name)?;
                let slug = match input.slug.as_deref() {
                    Some(slug) => normalize_non_empty_slug(slug)?,
                    None if input.name.is_some() => normalize_non_empty_slug(&name)?,
                    None => existing.slug.clone(),
                };
                ensure_translation_slug_available_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    &module_scope,
                    &locale,
                    &slug,
                    Some(term_id),
                )
                .await?;

                let revision = next_translation_revision(term_id, &locale, existing.revision)?;
                let updated = taxonomy_term_translation::Entity::update_many()
                    .col_expr(taxonomy_term_translation::Column::Name, Expr::value(name.clone()))
                    .col_expr(taxonomy_term_translation::Column::Slug, Expr::value(slug.clone()))
                    .col_expr(
                        taxonomy_term_translation::Column::Revision,
                        Expr::value(revision),
                    )
                    .col_expr(
                        taxonomy_term_translation::Column::UpdatedAt,
                        Expr::value(now.fixed_offset()),
                    )
                    .filter(taxonomy_term_translation::Column::Id.eq(existing.id))
                    .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
                    .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
                    .filter(taxonomy_term_translation::Column::Revision.eq(existing.revision))
                    .exec(txn)
                    .await?;
                if updated.rows_affected != 1 {
                    return Err(TaxonomyError::conflict(
                        "taxonomy term translation changed before the module update could commit",
                    ));
                }

                (
                    existing.id,
                    name,
                    slug,
                    revision,
                    DateTime::<Utc>::from(existing.created_at),
                )
            }
            None => {
                let name = input.name.clone().ok_or_else(|| {
                    TaxonomyError::validation("Name is required when adding a new locale")
                })?;
                validate_term_name(&name)?;
                let slug = normalize_non_empty_slug(input.slug.as_deref().unwrap_or(&name))?;
                ensure_translation_slug_available_in_tx(
                    txn,
                    tenant_id,
                    kind,
                    &module_scope,
                    &locale,
                    &slug,
                    Some(term_id),
                )
                .await?;

                let translation_id = Uuid::new_v4();
                taxonomy_term_translation::ActiveModel {
                    id: Set(translation_id),
                    term_id: Set(term_id),
                    tenant_id: Set(tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(name.clone()),
                    slug: Set(slug.clone()),
                    description: Set(None),
                    revision: Set(1),
                    created_at: Set(now.fixed_offset()),
                    updated_at: Set(now.fixed_offset()),
                }
                .insert(txn)
                .await?;

                (translation_id, name, slug, 1, now)
            }
        };

    let resource_revision = next_term_revision(&term)?;
    let updated = taxonomy_term::Entity::update_many()
        .col_expr(taxonomy_term::Column::Revision, Expr::value(resource_revision))
        .col_expr(
            taxonomy_term::Column::UpdatedAt,
            Expr::value(now.fixed_offset()),
        )
        .filter(taxonomy_term::Column::Id.eq(term_id))
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(kind))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .filter(taxonomy_term::Column::Revision.eq(term.revision))
        .exec(txn)
        .await?;
    if updated.rows_affected != 1 {
        return Err(TaxonomyError::conflict(
            "taxonomy term changed before the module update could commit",
        ));
    }

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

    let _ = translation_id;
    Ok(ModuleTermMutationResult {
        id: term_id,
        tenant_id,
        locale: locale.clone(),
        effective_locale: locale,
        name,
        slug,
        created_at,
    })
}

pub async fn delete_module_term_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    security: &SecurityContext,
    kind: TaxonomyTermKind,
    module_slug: &str,
) -> TaxonomyResult<()> {
    enforce_scope(security, Resource::Taxonomy, Action::Delete)?;

    let module_scope = normalize_module_scope(module_slug)?;
    let term = find_module_term_in_tx(txn, tenant_id, term_id, kind, &module_scope).await?;
    let translations = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?;
    let deletion_translation = translations.iter().min_by(|left, right| {
        left.locale
            .cmp(&right.locale)
            .then_with(|| left.id.cmp(&right.id))
    });
    let locale = deletion_translation
        .map(|translation| translation.locale.as_str())
        .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
    let target_revision = deletion_translation
        .map(|translation| translation.revision)
        .unwrap_or_default();
    let resource_revision = next_term_revision(&term)?;

    record_translation_change_in_tx(
        txn,
        TranslationChangeEvidence {
            tenant_id,
            term_id,
            locale,
            resource_revision,
            target_revision,
            operation: "delete",
            lifecycle: "deleted",
        },
    )
    .await?;

    let deleted = taxonomy_term::Entity::delete_many()
        .filter(taxonomy_term::Column::Id.eq(term_id))
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(kind))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .filter(taxonomy_term::Column::Revision.eq(term.revision))
        .exec(txn)
        .await?;
    if deleted.rows_affected != 1 {
        return Err(TaxonomyError::conflict(
            "taxonomy term changed before the module deletion could commit",
        ));
    }

    Ok(())
}

async fn find_module_term_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    kind: TaxonomyTermKind,
    module_scope: &str,
) -> TaxonomyResult<taxonomy_term::Model> {
    taxonomy_term::Entity::find_by_id(term_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(kind))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(module_scope))
        .one(txn)
        .await?
        .ok_or(TaxonomyError::TermNotFound(term_id))
}

async fn ensure_translation_slug_available_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    module_scope: &str,
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
        .filter(taxonomy_term::Column::Kind.eq(kind))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(module_scope));
    if let Some(exclude_term_id) = exclude_term_id {
        select = select.filter(taxonomy_term_translation::Column::TermId.ne(exclude_term_id));
    }
    if select.one(txn).await?.is_some() {
        return Err(TaxonomyError::DuplicateSlug(slug.to_string()));
    }
    Ok(())
}

fn enforce_scope(
    security: &SecurityContext,
    resource: Resource,
    action: Action,
) -> TaxonomyResult<()> {
    if matches!(security.get_scope(resource, action), PermissionScope::None) {
        return Err(TaxonomyError::forbidden("Permission denied"));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> TaxonomyResult<String> {
    normalize_locale_code(locale).ok_or_else(|| TaxonomyError::validation("Invalid locale"))
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

fn normalize_non_empty_slug(value: &str) -> TaxonomyResult<String> {
    let slug = slug::slugify(value);
    if slug.is_empty() {
        return Err(TaxonomyError::validation(
            "Localized slug cannot be empty after normalization",
        ));
    }
    Ok(slug)
}

fn validate_term_name(name: &str) -> TaxonomyResult<()> {
    if name.trim().is_empty() {
        return Err(TaxonomyError::validation("Term name cannot be empty"));
    }
    if name.chars().count() > 120 {
        return Err(TaxonomyError::validation(
            "Term name cannot exceed 120 characters",
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
