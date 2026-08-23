use std::collections::{BTreeSet, HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseTransaction, EntityTrait, QueryFilter, Statement, sea_query::Expr,
};
use uuid::Uuid;

use crate::{
    MAX_TAXONOMY_CATEGORY_DEPTH, TaxonomyError, TaxonomyResult, TaxonomyScopeType,
    TaxonomyTermKind,
    entities::{
        taxonomy_category_hierarchy, taxonomy_category_presentation, taxonomy_term,
        taxonomy_term_alias, taxonomy_term_translation,
    },
    normalize_taxonomy_category_color, normalize_taxonomy_category_icon_key, normalize_term_locale,
    normalize_term_route_key,
    route_key_registry::{ensure_route_key_available_in_tx, reconcile_route_keys_for_locale_in_tx},
    translation_evidence::{TranslationChangeEvidence, record_translation_change_in_tx},
};

/// Exact canonical Category snapshot supplied by a module that already owns and
/// authorizes the consumer command being mirrored into Taxonomy.
///
/// This port is intentionally module-scoped and transaction-bound. It exists so
/// staged consumer cutovers can keep legacy compatibility storage and canonical
/// Taxonomy storage consistent in one database transaction without teaching the
/// consumer about Taxonomy persistence tables. It does not authorize the caller:
/// the consumer must enforce its own domain permission before invoking it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncModuleCategoryInput {
    pub category_id: Uuid,
    pub module_scope: String,
    pub canonical_key: String,
    pub locale: String,
    pub name: String,
    pub slug: String,
    /// Complete append-only historical route set for this locale.
    pub aliases: Vec<String>,
    pub description: Option<String>,
    pub parent_id: Option<Uuid>,
    pub position: i32,
    /// Exact legacy-compatible icon/color snapshot. Media references are never
    /// changed by this compatibility sync port.
    pub icon_key: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncModuleCategoryResult {
    pub category_id: Uuid,
    pub resource_revision: i64,
    pub translation_revision: i64,
    pub presentation_revision: i64,
}

/// Synchronize one module-owned Category snapshot into canonical Taxonomy state.
///
/// The operation is idempotent and runs inside the caller transaction. Existing
/// Taxonomy identity must match the exact tenant/module/category/canonical-key
/// ownership boundary; incompatible UUID reuse fails closed. Localized copy and
/// append-only aliases advance Taxonomy Translation evidence only when changed.
/// Hierarchy is validated against Taxonomy's shared cycle/depth rules. Icon/color
/// updates preserve any Media-owned image/cover references already attached to
/// the canonical Category.
pub async fn sync_module_category_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    input: SyncModuleCategoryInput,
) -> TaxonomyResult<SyncModuleCategoryResult> {
    if input.category_id.is_nil() {
        return Err(TaxonomyError::validation(
            "Category owner sync requires a non-nil category identity",
        ));
    }
    if input.position < 0 {
        return Err(TaxonomyError::validation(
            "Category position must be zero or greater",
        ));
    }
    if input.parent_id == Some(input.category_id) {
        return Err(TaxonomyError::validation(
            "Category cannot be its own parent",
        ));
    }

    let module_scope = normalize_module_scope(&input.module_scope)?;
    let canonical_key = normalize_required_route_key(&input.canonical_key, "canonical key")?;
    let locale = normalize_term_locale(&input.locale)
        .ok_or_else(|| TaxonomyError::validation("Locale cannot be empty or invalid"))?;
    validate_term_name(&input.name)?;
    let slug = normalize_required_route_key(&input.slug, "localized slug")?;
    let description = normalize_optional_text(input.description.as_deref());
    validate_optional_description(description.as_deref())?;
    let aliases = normalize_aliases(&input.aliases, &slug)?;
    let icon_key = input
        .icon_key
        .as_deref()
        .map(normalize_taxonomy_category_icon_key)
        .transpose()?
        .flatten();
    let color = input
        .color
        .as_deref()
        .map(normalize_taxonomy_category_color)
        .transpose()?
        .flatten();

    serialize_category_hierarchy_writer(txn, tenant_id).await?;

    let (mut term, created_term) = ensure_category_identity(
        txn,
        tenant_id,
        input.category_id,
        &module_scope,
        &canonical_key,
    )
    .await?;

    if let Some(parent_id) = input.parent_id {
        ensure_parent_identity(txn, tenant_id, parent_id, &module_scope).await?;
    }

    ensure_route_key_available_in_tx(
        txn,
        tenant_id,
        TaxonomyTermKind::Category,
        TaxonomyScopeType::Module,
        &module_scope,
        &locale,
        &slug,
        Some(input.category_id),
    )
    .await?;
    for alias in &aliases {
        match ensure_route_key_available_in_tx(
            txn,
            tenant_id,
            TaxonomyTermKind::Category,
            TaxonomyScopeType::Module,
            &module_scope,
            &locale,
            alias,
            Some(input.category_id),
        )
        .await
        {
            Ok(()) => {}
            Err(TaxonomyError::DuplicateSlug(_)) => {
                return Err(TaxonomyError::DuplicateAlias(alias.clone()));
            }
            Err(error) => return Err(error),
        }
    }

    let existing_translation = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::TermId.eq(input.category_id))
        .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
        .one(txn)
        .await?;

    let now = Utc::now().fixed_offset();
    let (translation_revision, translation_changed) = match existing_translation {
        Some(existing)
            if existing.name == input.name
                && existing.slug == slug
                && existing.description == description =>
        {
            (existing.revision, false)
        }
        Some(existing) => {
            let revision = next_positive_revision(
                existing.revision,
                "Category translation revision is invalid or exhausted",
            )?;
            let updated = taxonomy_term_translation::Entity::update_many()
                .col_expr(
                    taxonomy_term_translation::Column::Name,
                    Expr::value(input.name.clone()),
                )
                .col_expr(
                    taxonomy_term_translation::Column::Slug,
                    Expr::value(slug.clone()),
                )
                .col_expr(
                    taxonomy_term_translation::Column::Description,
                    Expr::value(description.clone()),
                )
                .col_expr(
                    taxonomy_term_translation::Column::Revision,
                    Expr::value(revision),
                )
                .col_expr(
                    taxonomy_term_translation::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(taxonomy_term_translation::Column::Id.eq(existing.id))
                .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_translation::Column::TermId.eq(input.category_id))
                .filter(taxonomy_term_translation::Column::Revision.eq(existing.revision))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(TaxonomyError::conflict(
                    "Category localized copy changed before owner sync could commit",
                ));
            }
            (revision, true)
        }
        None => {
            taxonomy_term_translation::ActiveModel {
                id: Set(Uuid::new_v4()),
                term_id: Set(input.category_id),
                tenant_id: Set(tenant_id),
                locale: Set(locale.clone()),
                name: Set(input.name.clone()),
                slug: Set(slug.clone()),
                description: Set(description.clone()),
                revision: Set(1),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(txn)
            .await?;
            (1, true)
        }
    };

    let aliases_changed = sync_append_only_aliases(
        txn,
        tenant_id,
        input.category_id,
        &locale,
        &aliases,
    )
    .await?;

    let resource_changed = created_term || translation_changed || aliases_changed;
    let resource_revision = if created_term {
        term.revision
    } else if resource_changed {
        let revision = next_positive_revision(
            term.revision,
            "Category resource revision is invalid or exhausted",
        )?;
        let updated = taxonomy_term::Entity::update_many()
            .col_expr(taxonomy_term::Column::Revision, Expr::value(revision))
            .col_expr(taxonomy_term::Column::UpdatedAt, Expr::value(now))
            .filter(taxonomy_term::Column::Id.eq(input.category_id))
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Revision.eq(term.revision))
            .exec(txn)
            .await?;
        if updated.rows_affected != 1 {
            return Err(TaxonomyError::conflict(
                "Category identity changed before owner sync could commit",
            ));
        }
        term.revision = revision;
        revision
    } else {
        term.revision
    };

    if resource_changed {
        record_translation_change_in_tx(
            txn,
            TranslationChangeEvidence {
                tenant_id,
                term_id: input.category_id,
                locale: &locale,
                resource_revision,
                target_revision: translation_revision,
                operation: "upsert",
            },
        )
        .await?;
    } else {
        // Keep the route registry self-healing without manufacturing Translation
        // change evidence for an otherwise idempotent compatibility sync.
        reconcile_route_keys_for_locale_in_tx(txn, tenant_id, input.category_id, &locale).await?;
    }

    sync_category_hierarchy(
        txn,
        tenant_id,
        input.category_id,
        input.parent_id,
        input.position,
    )
    .await?;
    let presentation_revision = sync_category_presentation(
        txn,
        tenant_id,
        input.category_id,
        icon_key,
        color,
    )
    .await?;

    Ok(SyncModuleCategoryResult {
        category_id: input.category_id,
        resource_revision,
        translation_revision,
        presentation_revision,
    })
}

async fn ensure_category_identity(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
    canonical_key: &str,
) -> TaxonomyResult<(taxonomy_term::Model, bool)> {
    if let Some(existing) = taxonomy_term::Entity::find_by_id(category_id)
        .one(txn)
        .await?
    {
        if existing.tenant_id != tenant_id
            || existing.kind != TaxonomyTermKind::Category
            || existing.scope_type != TaxonomyScopeType::Module
            || existing.scope_value != module_scope
            || existing.canonical_key != canonical_key
        {
            return Err(TaxonomyError::conflict(format!(
                "Category UUID {category_id} is already owned by an incompatible Taxonomy term",
            )));
        }
        if existing.revision <= 0 {
            return Err(TaxonomyError::conflict(format!(
                "Category {category_id} has an invalid resource revision",
            )));
        }
        return Ok((existing, false));
    }

    if taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(module_scope))
        .filter(taxonomy_term::Column::CanonicalKey.eq(canonical_key))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(TaxonomyError::DuplicateCanonicalKey(
            canonical_key.to_owned(),
        ));
    }

    let now = Utc::now().fixed_offset();
    let created = taxonomy_term::ActiveModel {
        id: Set(category_id),
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Category),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(module_scope.to_owned()),
        canonical_key: Set(canonical_key.to_owned()),
        revision: Set(1),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(txn)
    .await?;
    Ok((created, true))
}

async fn ensure_parent_identity(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Uuid,
    module_scope: &str,
) -> TaxonomyResult<()> {
    let parent = taxonomy_term::Entity::find_by_id(parent_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(module_scope))
        .one(txn)
        .await?;
    if parent.is_none() {
        return Err(TaxonomyError::validation(format!(
            "Category parent {parent_id} must exist in the same tenant and module scope",
        )));
    }
    Ok(())
}

async fn sync_append_only_aliases(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    locale: &str,
    desired_aliases: &[String],
) -> TaxonomyResult<bool> {
    let existing = taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::TermId.eq(term_id))
        .filter(taxonomy_term_alias::Column::Locale.eq(locale))
        .all(txn)
        .await?;
    let existing_slugs = existing
        .iter()
        .map(|alias| alias.slug.clone())
        .collect::<BTreeSet<_>>();
    let desired = desired_aliases.iter().cloned().collect::<BTreeSet<_>>();

    let missing_from_snapshot = existing_slugs.difference(&desired).next().cloned();
    if let Some(alias) = missing_from_snapshot {
        return Err(TaxonomyError::conflict(format!(
            "Category owner sync cannot remove append-only historical route {locale}/{alias}",
        )));
    }

    let new_aliases = desired
        .difference(&existing_slugs)
        .cloned()
        .collect::<Vec<_>>();
    if new_aliases.is_empty() {
        return Ok(false);
    }

    let now = Utc::now().fixed_offset();
    for alias in new_aliases {
        taxonomy_term_alias::ActiveModel {
            id: Set(Uuid::new_v4()),
            term_id: Set(term_id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.to_owned()),
            name: Set(alias.clone()),
            slug: Set(alias),
            created_at: Set(now),
        }
        .insert(txn)
        .await?;
    }
    Ok(true)
}

async fn sync_category_hierarchy(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
) -> TaxonomyResult<()> {
    let rows = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .all(txn)
        .await?;
    validate_candidate_hierarchy(term_id, parent_id, &rows)?;

    match taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, term_id))
        .one(txn)
        .await?
    {
        Some(existing) if existing.parent_term_id == parent_id && existing.position == position => {
            Ok(())
        }
        Some(existing) => {
            let mut active: taxonomy_category_hierarchy::ActiveModel = existing.into();
            active.parent_term_id = Set(parent_id);
            active.position = Set(position);
            active.update(txn).await?;
            Ok(())
        }
        None => {
            taxonomy_category_hierarchy::ActiveModel {
                tenant_id: Set(tenant_id),
                term_id: Set(term_id),
                parent_term_id: Set(parent_id),
                position: Set(position),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
    }
}

fn validate_candidate_hierarchy(
    term_id: Uuid,
    parent_id: Option<Uuid>,
    rows: &[taxonomy_category_hierarchy::Model],
) -> TaxonomyResult<()> {
    let mut parent_by_child = rows
        .iter()
        .map(|row| (row.term_id, row.parent_term_id))
        .collect::<HashMap<_, _>>();
    parent_by_child.insert(term_id, parent_id);

    for start in parent_by_child.keys().copied() {
        let mut seen = HashSet::new();
        let mut current = Some(start);
        let mut depth = 0usize;
        while let Some(node) = current {
            if !seen.insert(node) {
                return Err(TaxonomyError::validation(
                    "Category hierarchy cannot contain a cycle",
                ));
            }
            current = parent_by_child.get(&node).copied().flatten();
            if current.is_some() {
                depth += 1;
                if depth > MAX_TAXONOMY_CATEGORY_DEPTH {
                    return Err(TaxonomyError::validation(format!(
                        "Category hierarchy cannot exceed depth {MAX_TAXONOMY_CATEGORY_DEPTH}",
                    )));
                }
            }
        }
    }
    Ok(())
}

async fn sync_category_presentation(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    icon_key: Option<String>,
    color: Option<String>,
) -> TaxonomyResult<i64> {
    let existing = taxonomy_category_presentation::Entity::find_by_id((tenant_id, term_id))
        .one(txn)
        .await?;
    match existing {
        None if icon_key.is_none() && color.is_none() => Ok(0),
        None => {
            let now = Utc::now().fixed_offset();
            taxonomy_category_presentation::ActiveModel {
                tenant_id: Set(tenant_id),
                term_id: Set(term_id),
                icon_key: Set(icon_key),
                color: Set(color),
                image_media_id: Set(None),
                cover_media_id: Set(None),
                revision: Set(1),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(txn)
            .await?;
            Ok(1)
        }
        Some(existing) if existing.icon_key == icon_key && existing.color == color => {
            Ok(existing.revision)
        }
        Some(existing) => {
            let revision = next_positive_revision(
                existing.revision,
                "Category presentation revision is invalid or exhausted",
            )?;
            let now = Utc::now().fixed_offset();
            let updated = taxonomy_category_presentation::Entity::update_many()
                .col_expr(
                    taxonomy_category_presentation::Column::IconKey,
                    Expr::value(icon_key),
                )
                .col_expr(
                    taxonomy_category_presentation::Column::Color,
                    Expr::value(color),
                )
                .col_expr(
                    taxonomy_category_presentation::Column::Revision,
                    Expr::value(revision),
                )
                .col_expr(
                    taxonomy_category_presentation::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(taxonomy_category_presentation::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_category_presentation::Column::TermId.eq(term_id))
                .filter(taxonomy_category_presentation::Column::Revision.eq(existing.revision))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(TaxonomyError::conflict(
                    "Category presentation changed before owner sync could commit",
                ));
            }
            Ok(revision)
        }
    }
}

async fn serialize_category_hierarchy_writer(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> TaxonomyResult<()> {
    if txn.get_database_backend() == DatabaseBackend::Postgres {
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![tenant_id.to_string().into()],
        ))
        .await?;
    }
    Ok(())
}

fn normalize_module_scope(value: &str) -> TaxonomyResult<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
        .collect::<String>();
    if normalized.is_empty() {
        return Err(TaxonomyError::validation(
            "Module scope requires a non-empty scope value",
        ));
    }
    Ok(normalized)
}

fn normalize_required_route_key(value: &str, field: &str) -> TaxonomyResult<String> {
    normalize_term_route_key(value)
        .ok_or_else(|| TaxonomyError::validation(format!("Category {field} cannot be empty")))
}

fn normalize_aliases(values: &[String], current_slug: &str) -> TaxonomyResult<Vec<String>> {
    let mut aliases = BTreeSet::new();
    for value in values {
        let alias = normalize_required_route_key(value, "alias")?;
        if alias == current_slug {
            return Err(TaxonomyError::validation(
                "Category historical alias cannot equal the current localized slug",
            ));
        }
        aliases.insert(alias);
    }
    Ok(aliases.into_iter().collect())
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

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_optional_description(value: Option<&str>) -> TaxonomyResult<()> {
    if value.is_some_and(|description| description.chars().count() > 2_000) {
        return Err(TaxonomyError::validation(
            "Description cannot exceed 2000 characters",
        ));
    }
    Ok(())
}

fn next_positive_revision(current: i64, message: &str) -> TaxonomyResult<i64> {
    current
        .checked_add(1)
        .filter(|next| current > 0 && *next > 0)
        .ok_or_else(|| TaxonomyError::conflict(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_scope_and_aliases_are_normalized_deterministically() {
        assert_eq!(normalize_module_scope(" Forum! ").unwrap(), "forum");
        assert_eq!(
            normalize_aliases(
                &[" Old Route ".to_owned(), "old-route".to_owned()],
                "current-route"
            )
            .unwrap(),
            vec!["old-route"]
        );
        assert!(
            normalize_aliases(&["current route".to_owned()], "current-route").is_err()
        );
    }
}
