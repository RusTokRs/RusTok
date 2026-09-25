use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use crate::{
    TaxonomyError, TaxonomyResult, TaxonomyScopeType, TaxonomyTermKind,
    category_hierarchy::validate_candidate_hierarchy,
    entities::{taxonomy_category_hierarchy, taxonomy_term},
    lock_category_hierarchy_writer_in_tx,
};

/// Move one module-owned Category and normalize both affected sibling sets.
///
/// Taxonomy owns hierarchy validation, cycle/depth enforcement and sibling persistence. The
/// caller supplies the domain-authorized target and receives the exact placements rewritten by
/// this transaction. No consumer persistence is accessed here.
pub async fn move_module_category_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
    new_parent_id: Option<Uuid>,
    new_position: i32,
) -> TaxonomyResult<Vec<crate::dto::TaxonomyCategoryPlacement>> {
    if new_position < 0 {
        return Err(TaxonomyError::validation(
            "Module Category sibling position must be zero or greater",
        ));
    }
    let module_scope = normalize_module_scope(module_scope)?;
    lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?;
    ensure_category_in_module_scope(txn, tenant_id, category_id, &module_scope).await?;
    if let Some(parent_id) = new_parent_id {
        ensure_category_in_module_scope(txn, tenant_id, parent_id, &module_scope).await?;
    }

    let scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .all(txn)
        .await?;
    let scoped_ids = scoped_terms.iter().map(|term| term.id).collect::<HashSet<_>>();
    let hierarchy_rows = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.is_in(scoped_ids.iter().copied()))
        .lock_exclusive()
        .all(txn)
        .await?;
    if hierarchy_rows.len() != scoped_terms.len() {
        return Err(TaxonomyError::invariant(
            "Module Category hierarchy coverage is incomplete during move",
        ));
    }
    let current = hierarchy_rows.iter().find(|row| row.term_id == category_id).ok_or_else(|| {
        TaxonomyError::invariant(format!(
            "Module Category {category_id} hierarchy placement is missing during move"
        ))
    })?;
    let source_parent_id = current.parent_term_id;
    validate_candidate_hierarchy(category_id, new_parent_id, &hierarchy_rows)?;

    let mut source_siblings = hierarchy_rows
        .iter()
        .filter(|row| row.parent_term_id == source_parent_id && row.term_id != category_id)
        .map(|row| (row.term_id, row.position))
        .collect::<Vec<_>>();
    source_siblings.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    let mut target_siblings = if source_parent_id == new_parent_id {
        source_siblings.clone()
    } else {
        hierarchy_rows
            .iter()
            .filter(|row| row.parent_term_id == new_parent_id && row.term_id != category_id)
            .map(|row| (row.term_id, row.position))
            .collect::<Vec<_>>()
    };
    if source_parent_id != new_parent_id {
        target_siblings.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
    }
    let target_position = usize::try_from(new_position).map_err(|_| {
        TaxonomyError::validation("Module Category sibling position exceeds usize range")
    })?;
    if target_position > target_siblings.len() {
        return Err(TaxonomyError::validation(format!(
            "Module Category position {} exceeds destination sibling count {}",
            new_position, target_siblings.len()
        )));
    }
    target_siblings.insert(target_position, (category_id, new_position));

    let mut affected = Vec::<(Uuid, Option<Uuid>, i32)>::new();
    if source_parent_id != new_parent_id {
        for (position, (term_id, _)) in source_siblings.iter().copied().enumerate() {
            let position = i32::try_from(position).map_err(|_| {
                TaxonomyError::validation("Module Category sibling position exceeds i32 range")
            })?;
            affected.push((term_id, source_parent_id, position));
        }
    }
    for (position, (term_id, _)) in target_siblings.iter().copied().enumerate() {
        let position = i32::try_from(position).map_err(|_| {
            TaxonomyError::validation("Module Category sibling position exceeds i32 range")
        })?;
        affected.push((term_id, new_parent_id, position));
    }

    let mut rows_by_id = hierarchy_rows.into_iter().map(|row| (row.term_id, row)).collect::<HashMap<_, _>>();
    let mut placements = Vec::with_capacity(affected.len());
    for (term_id, parent_id, position) in affected {
        let row = rows_by_id.remove(&term_id).ok_or_else(|| {
            TaxonomyError::invariant(format!(
                "Module Category hierarchy placement disappeared for {term_id} during move"
            ))
        })?;
        if row.parent_term_id == parent_id && row.position == position {
            placements.push(crate::dto::TaxonomyCategoryPlacement { term_id, parent_id, position });
            continue;
        }
        let mut active: taxonomy_category_hierarchy::ActiveModel = row.into();
        active.parent_term_id = Set(parent_id);
        active.position = Set(position);
        let row = active.update(txn).await?;
        placements.push(crate::dto::TaxonomyCategoryPlacement {
            term_id: row.term_id,
            parent_id: row.parent_term_id,
            position: row.position,
        });
    }
    Ok(placements)
}
/// Reorder every canonical sibling in a module-owned Category scope.
///
/// The caller owns authorization and the surrounding transaction. Taxonomy owns
/// hierarchy persistence and therefore validates the complete sibling set before
/// mutating any placement. Partial consumer-owned reorder snapshots fail closed.
pub async fn reorder_module_category_siblings_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    module_scope: &str,
    parent_id: Option<Uuid>,
    ordered_term_ids: &[Uuid],
) -> TaxonomyResult<()> {
    let module_scope = normalize_module_scope(module_scope)?;
    if ordered_term_ids
        .iter()
        .collect::<HashSet<_>>()
        .len()
        != ordered_term_ids.len()
    {
        return Err(TaxonomyError::validation(
            "Module Category sibling order contains duplicate identities",
        ));
    }

    lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?;

    if let Some(parent_id) = parent_id {
        ensure_category_in_module_scope(txn, tenant_id, parent_id, &module_scope).await?;
    }

    let scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .all(txn)
        .await?;
    let scoped_ids = scoped_terms.iter().map(|term| term.id).collect::<HashSet<_>>();

    if ordered_term_ids
        .iter()
        .any(|term_id| !scoped_ids.contains(term_id))
    {
        return Err(TaxonomyError::conflict(
            "Module Category sibling order contains an identity outside the owning scope",
        ));
    }

    let hierarchy_rows = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.is_in(scoped_ids.iter().copied()))
        .lock_exclusive()
        .all(txn)
        .await?;
    if hierarchy_rows.len() != scoped_terms.len() {
        return Err(TaxonomyError::invariant(
            "Module Category hierarchy coverage is incomplete",
        ));
    }

    let sibling_ids = hierarchy_rows
        .iter()
        .filter(|row| row.parent_term_id == parent_id)
        .map(|row| row.term_id)
        .collect::<HashSet<_>>();
    let ordered_ids = ordered_term_ids.iter().copied().collect::<HashSet<_>>();
    if !sibling_ids.is_subset(&ordered_ids) {
        return Err(TaxonomyError::conflict(
            "Module Category sibling order does not cover the complete canonical sibling set",
        ));
    }

    let mut rows_by_id = hierarchy_rows
        .into_iter()
        .map(|row| (row.term_id, row))
        .collect::<HashMap<_, _>>();

    for (position, term_id) in ordered_term_ids.iter().copied().enumerate() {
        let position = i32::try_from(position).map_err(|_| {
            TaxonomyError::validation("Module Category sibling position exceeds i32 range")
        })?;
        let row = rows_by_id.remove(&term_id).ok_or_else(|| {
            TaxonomyError::invariant(format!(
                "Module Category hierarchy placement missing for sibling {term_id}"
            ))
        })?;
        if row.parent_term_id == parent_id && row.position == position {
            continue;
        }

        let mut active: taxonomy_category_hierarchy::ActiveModel = row.into();
        active.parent_term_id = Set(parent_id);
        active.position = Set(position);
        active.update(txn).await?;
    }

    Ok(())
}

/// Shift existing siblings for a just-created module-owned Category insertion.
///
/// The caller must have already created the Category identity and placement at
/// requested_position in the same transaction. Taxonomy owns sibling persistence and
/// serializes the shift under the canonical tenant hierarchy lock.
pub async fn shift_module_category_siblings_for_insert_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
    parent_id: Option<Uuid>,
    requested_position: i32,
) -> TaxonomyResult<()> {
    if requested_position < 0 {
        return Err(TaxonomyError::validation(
            "Module Category sibling position must be zero or greater",
        ));
    }
    let module_scope = normalize_module_scope(module_scope)?;
    lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?;
    ensure_category_in_module_scope(txn, tenant_id, category_id, &module_scope).await?;
    if let Some(parent_id) = parent_id {
        ensure_category_in_module_scope(txn, tenant_id, parent_id, &module_scope).await?;
    }
    let placement = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, category_id))
        .one(txn)
        .await?
        .ok_or_else(|| TaxonomyError::invariant(format!(
            "Module Category {category_id} has no canonical hierarchy placement after owner sync"
        )))?;
    if placement.parent_term_id != parent_id || placement.position != requested_position {
        return Err(TaxonomyError::conflict(
            "Module Category insertion placement changed before sibling normalization",
        ));
    }
    let siblings = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(match parent_id {
            Some(parent_id) => taxonomy_category_hierarchy::Column::ParentTermId.eq(parent_id),
            None => taxonomy_category_hierarchy::Column::ParentTermId.is_null(),
        })
        .filter(taxonomy_category_hierarchy::Column::Position.gte(requested_position))
        .filter(taxonomy_category_hierarchy::Column::TermId.ne(category_id))
        .order_by_desc(taxonomy_category_hierarchy::Column::Position)
        .order_by_desc(taxonomy_category_hierarchy::Column::TermId)
        .lock_exclusive()
        .all(txn)
        .await?;
    for sibling in siblings {
        let next_position = sibling.position.checked_add(1).ok_or_else(|| {
            TaxonomyError::validation("Module Category sibling position exceeds i32 range")
        })?;
        let mut active: taxonomy_category_hierarchy::ActiveModel = sibling.into();
        active.position = Set(next_position);
        active.update(txn).await?;
    }
    Ok(())
}
/// Remove one module-owned Category placement and compact its former siblings.
///
/// Canonical Category identity remains in Taxonomy and is deleted by the outer
/// owner delete operation. This helper owns only the hierarchy mutation so the
/// consumer cannot bypass the Taxonomy storage boundary.
pub async fn delete_module_category_placement_and_compact_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
) -> TaxonomyResult<()> {
    let module_scope = normalize_module_scope(module_scope)?;

    lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?;

    ensure_category_in_module_scope(txn, tenant_id, category_id, &module_scope).await?;

    let placement = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, category_id))
        .lock_exclusive()
        .one(txn)
        .await?
        .ok_or_else(|| {
            TaxonomyError::invariant(format!(
                "Module Category {category_id} has no canonical hierarchy placement"
            ))
        })?;

    if taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::ParentTermId.eq(category_id))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(TaxonomyError::validation(
            "Category must be a leaf before deletion; move or delete its children first",
        ));
    }

    let parent_id = placement.parent_term_id;
    let deleted = taxonomy_category_hierarchy::Entity::delete_by_id((tenant_id, category_id))
        .exec(txn)
        .await?;
    if deleted.rows_affected != 1 {
        return Err(TaxonomyError::conflict(
            "Module Category hierarchy placement changed before deletion could commit",
        ));
    }

    let scoped_terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(&module_scope))
        .filter(taxonomy_term::Column::Id.ne(category_id))
        .all(txn)
        .await?;
    let remaining_ids = scoped_terms.iter().map(|term| term.id).collect::<Vec<_>>();

    if remaining_ids.is_empty() {
        return Ok(());
    }

    let siblings = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.is_in(remaining_ids))
        .filter(match parent_id {
            Some(parent_id) => taxonomy_category_hierarchy::Column::ParentTermId.eq(parent_id),
            None => taxonomy_category_hierarchy::Column::ParentTermId.is_null(),
        })
        .order_by_asc(taxonomy_category_hierarchy::Column::Position)
        .order_by_asc(taxonomy_category_hierarchy::Column::TermId)
        .lock_exclusive()
        .all(txn)
        .await?;

    let expected_siblings = siblings.len();
    let remaining_hierarchy = taxonomy_category_hierarchy::Entity::find()
        .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_category_hierarchy::Column::TermId.is_in(
            scoped_terms.iter().map(|term| term.id).collect::<Vec<_>>(),
        ))
        .all(txn)
        .await?;
    if remaining_hierarchy.len() != scoped_terms.len() {
        return Err(TaxonomyError::invariant(
            "Module Category hierarchy coverage is incomplete during deletion",
        ));
    }
    if expected_siblings == 0 {
        return Ok(());
    }

    for (position, sibling) in siblings.into_iter().enumerate() {
        let position = i32::try_from(position).map_err(|_| {
            TaxonomyError::invariant("Module Category sibling position exceeds i32 range")
        })?;
        if sibling.position == position {
            continue;
        }
        let mut active: taxonomy_category_hierarchy::ActiveModel = sibling.into();
        active.position = Set(position);
        active.update(txn).await?;
    }

    Ok(())
}

async fn ensure_category_in_module_scope(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    module_scope: &str,
) -> TaxonomyResult<()> {
    let term = taxonomy_term::Entity::find_by_id(category_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(module_scope))
        .one(txn)
        .await?;
    if term.is_none() {
        return Err(TaxonomyError::TermNotFound(category_id));
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
