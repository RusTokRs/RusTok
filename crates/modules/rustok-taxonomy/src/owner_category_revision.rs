use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, sea_query::Expr,
};
use uuid::Uuid;

use crate::{TaxonomyError, TaxonomyResult, TaxonomyTermKind, entities::taxonomy_term};

pub const MAX_TAXONOMY_CATEGORY_OWNER_PAGE: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxonomyCategoryOwnerRevision {
    pub category_id: Uuid,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonomyCategoryOwnerRevisionPage {
    pub categories: Vec<TaxonomyCategoryOwnerRevision>,
    pub next_after: Option<Uuid>,
}

/// List the canonical Category owner inventory in stable UUID order without exposing
/// Taxonomy persistence rows to a host adapter.
///
/// This is intentionally independent from consumer scope. `taxonomy.category` is one
/// Flex donor type across global and module Category scopes, so Translation inventory
/// must be derived from canonical Taxonomy existence rather than whichever attached
/// Flex rows happen to exist.
pub async fn list_category_owner_revisions_in<C>(
    connection: &C,
    tenant_id: Uuid,
    after: Option<Uuid>,
    limit: u16,
) -> TaxonomyResult<TaxonomyCategoryOwnerRevisionPage>
where
    C: ConnectionTrait,
{
    if limit == 0 || limit > MAX_TAXONOMY_CATEGORY_OWNER_PAGE {
        return Err(TaxonomyError::validation(format!(
            "Category owner page size must be between 1 and {MAX_TAXONOMY_CATEGORY_OWNER_PAGE}"
        )));
    }

    let mut query = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .order_by_asc(taxonomy_term::Column::Id);
    if let Some(after) = after {
        query = query.filter(taxonomy_term::Column::Id.gt(after));
    }

    let mut rows = query.limit(u64::from(limit) + 1).all(connection).await?;
    let has_more = rows.len() > usize::from(limit);
    if has_more {
        rows.truncate(usize::from(limit));
    }

    let categories = rows
        .into_iter()
        .map(|row| {
            validate_revision(row.id, row.revision)?;
            Ok(TaxonomyCategoryOwnerRevision {
                category_id: row.id,
                revision: row.revision,
            })
        })
        .collect::<TaxonomyResult<Vec<_>>>()?;
    let next_after = has_more
        .then(|| categories.last().map(|category| category.category_id))
        .flatten();

    Ok(TaxonomyCategoryOwnerRevisionPage {
        categories,
        next_after,
    })
}

/// Read owner revisions for a bounded set of Category identities without exposing
/// Taxonomy persistence rows to a host adapter. Missing/foreign/tag identities are omitted.
pub async fn load_category_owner_revisions_in<C>(
    connection: &C,
    tenant_id: Uuid,
    category_ids: &[Uuid],
) -> TaxonomyResult<Vec<TaxonomyCategoryOwnerRevision>>
where
    C: ConnectionTrait,
{
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::Id.is_in(category_ids.to_vec()))
        .order_by_asc(taxonomy_term::Column::Id)
        .all(connection)
        .await?;

    rows.into_iter()
        .map(|row| {
            validate_revision(row.id, row.revision)?;
            Ok(TaxonomyCategoryOwnerRevision {
                category_id: row.id,
                revision: row.revision,
            })
        })
        .collect()
}

/// Lock one canonical Category owner row as the serialization point for a host-composed
/// attached-value mutation. The caller keeps this transaction open while reading/writing
/// Flex-owned attached rows.
pub async fn lock_category_owner_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> TaxonomyResult<TaxonomyCategoryOwnerRevision> {
    let row = taxonomy_term::Entity::find_by_id(category_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .lock_exclusive()
        .one(txn)
        .await?
        .ok_or(TaxonomyError::TermNotFound(category_id))?;
    validate_revision(category_id, row.revision)?;
    Ok(TaxonomyCategoryOwnerRevision {
        category_id,
        revision: row.revision,
    })
}

/// Advance the canonical Category aggregate revision under compare-and-swap after an
/// attached-value mutation changed owner-visible state.
pub async fn advance_category_owner_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    expected_revision: i64,
) -> TaxonomyResult<i64> {
    validate_revision(category_id, expected_revision)?;
    let next_revision = expected_revision.checked_add(1).ok_or_else(|| {
        TaxonomyError::conflict(format!(
            "Category {category_id} resource revision is exhausted"
        ))
    })?;
    let updated = taxonomy_term::Entity::update_many()
        .col_expr(taxonomy_term::Column::Revision, Expr::value(next_revision))
        .col_expr(
            taxonomy_term::Column::UpdatedAt,
            Expr::value(Utc::now().fixed_offset()),
        )
        .filter(taxonomy_term::Column::Id.eq(category_id))
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::Revision.eq(expected_revision))
        .exec(txn)
        .await?;
    if updated.rows_affected != 1 {
        return Err(TaxonomyError::conflict(format!(
            "Category {category_id} changed before attached values could commit"
        )));
    }
    Ok(next_revision)
}

fn validate_revision(category_id: Uuid, revision: i64) -> TaxonomyResult<()> {
    if revision <= 0 {
        return Err(TaxonomyError::conflict(format!(
            "Category {category_id} has an invalid resource revision"
        )));
    }
    Ok(())
}
