use std::collections::BTreeSet;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
};
use uuid::Uuid;

use crate::{
    dto::{TaxonomyScopeType, TaxonomyTermKind},
    entities::{
        taxonomy_term, taxonomy_term_alias, taxonomy_term_route_key, taxonomy_term_translation,
    },
    error::{TaxonomyError, TaxonomyResult},
};

pub(crate) async fn ensure_route_key_available_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    scope_type: TaxonomyScopeType,
    scope_value: &str,
    locale: &str,
    route_key: &str,
    exclude_term_id: Option<Uuid>,
) -> TaxonomyResult<()> {
    let owner = taxonomy_term_route_key::Entity::find()
        .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_route_key::Column::Kind.eq(kind))
        .filter(taxonomy_term_route_key::Column::ScopeType.eq(scope_type))
        .filter(taxonomy_term_route_key::Column::ScopeValue.eq(scope_value))
        .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
        .filter(taxonomy_term_route_key::Column::RouteKey.eq(route_key))
        .one(txn)
        .await?;
    if owner.is_some_and(|owner| Some(owner.term_id) != exclude_term_id) {
        return Err(TaxonomyError::DuplicateSlug(route_key.to_string()));
    }
    Ok(())
}

pub(crate) async fn reconcile_route_keys_for_locale_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    locale: &str,
) -> TaxonomyResult<()> {
    let term = taxonomy_term::Entity::find_by_id(term_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(TaxonomyError::TermNotFound(term_id))?;

    let mut desired = BTreeSet::new();
    if let Some(translation) = taxonomy_term_translation::Entity::find()
        .filter(taxonomy_term_translation::Column::TermId.eq(term_id))
        .filter(taxonomy_term_translation::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_translation::Column::Locale.eq(locale))
        .one(txn)
        .await?
    {
        desired.insert(translation.slug);
    }
    for alias in taxonomy_term_alias::Entity::find()
        .filter(taxonomy_term_alias::Column::TermId.eq(term_id))
        .filter(taxonomy_term_alias::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_alias::Column::Locale.eq(locale))
        .all(txn)
        .await?
    {
        desired.insert(alias.slug);
    }

    let existing = taxonomy_term_route_key::Entity::find()
        .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_route_key::Column::TermId.eq(term_id))
        .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
        .all(txn)
        .await?;
    let existing_keys = existing
        .iter()
        .map(|route| route.route_key.clone())
        .collect::<BTreeSet<_>>();

    for route_key in desired.difference(&existing_keys) {
        match (taxonomy_term_route_key::ActiveModel {
            tenant_id: Set(tenant_id),
            kind: Set(term.kind),
            scope_type: Set(term.scope_type),
            scope_value: Set(term.scope_value.clone()),
            locale: Set(locale.to_string()),
            route_key: Set(route_key.clone()),
            term_id: Set(term_id),
        })
        .insert(txn)
        .await
        {
            Ok(_) => {}
            Err(error) if is_unique_constraint(&error) => {
                // PostgreSQL marks the transaction failed after a constraint
                // violation. Do not attempt a follow-up owner read here: the
                // caller must roll back the entire localized mutation.
                return Err(TaxonomyError::conflict(format!(
                    "localized taxonomy route key `{route_key}` for locale `{locale}` was claimed concurrently",
                )));
            }
            Err(error) => return Err(error.into()),
        }
    }

    let stale = existing_keys
        .difference(&desired)
        .cloned()
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        taxonomy_term_route_key::Entity::delete_many()
            .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term_route_key::Column::TermId.eq(term_id))
            .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
            .filter(taxonomy_term_route_key::Column::RouteKey.is_in(stale))
            .exec(txn)
            .await?;
    }

    Ok(())
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
