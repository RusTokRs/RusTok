use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

use crate::dto::{
    SetTaxonomyCategoryPlacementInput, TaxonomyCategoryPlacement, TaxonomyTermKind,
};
use crate::entities::{taxonomy_category_hierarchy, taxonomy_term};
use crate::error::{TaxonomyError, TaxonomyResult};
use crate::services::TaxonomyService;

/// Shared platform maximum for a category path, expressed as parent edges from a node to its root.
pub const MAX_TAXONOMY_CATEGORY_DEPTH: usize = 16;

impl TaxonomyService {
    /// Return the effective shared placement for a Category term.
    ///
    /// A Category without a persisted placement is a root at position `0`. This keeps the generic
    /// term creation path backwards-compatible while consumer migrations progressively materialize
    /// explicit hierarchy rows.
    pub async fn get_category_placement(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        term_id: Uuid,
    ) -> TaxonomyResult<TaxonomyCategoryPlacement> {
        enforce_taxonomy_scope(&security, Action::Read)?;
        let term = load_category(self.database(), tenant_id, term_id).await?;
        let row = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, term.id))
            .one(self.database())
            .await?;

        Ok(row
            .map(placement_from_model)
            .unwrap_or(TaxonomyCategoryPlacement {
                term_id,
                parent_id: None,
                position: 0,
            }))
    }

    /// Set a Category's parent and sibling position inside its Taxonomy-owned hierarchy.
    ///
    /// Parent and child must be Categories in the same tenant and Taxonomy scope. The resulting
    /// hierarchy is checked for cycles and the shared maximum depth before the placement is written.
    pub async fn set_category_placement(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        term_id: Uuid,
        input: SetTaxonomyCategoryPlacementInput,
    ) -> TaxonomyResult<TaxonomyCategoryPlacement> {
        enforce_taxonomy_scope(&security, Action::Update)?;
        if input.position < 0 {
            return Err(TaxonomyError::validation(
                "Category position must be zero or greater",
            ));
        }
        if input.parent_id == Some(term_id) {
            return Err(TaxonomyError::validation(
                "Category cannot be its own parent",
            ));
        }

        let txn = self.database().begin().await?;
        let term = load_category(&txn, tenant_id, term_id).await?;

        if let Some(parent_id) = input.parent_id {
            let parent = load_category(&txn, tenant_id, parent_id).await?;
            if parent.scope_type != term.scope_type || parent.scope_value != term.scope_value {
                return Err(TaxonomyError::validation(
                    "Category parent must use the same Taxonomy scope as its child",
                ));
            }
        }

        let rows = taxonomy_category_hierarchy::Entity::find()
            .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
            .all(&txn)
            .await?;
        validate_candidate_hierarchy(term_id, input.parent_id, &rows)?;

        let existing = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, term_id))
            .one(&txn)
            .await?;
        let row = match existing {
            Some(existing) => {
                let mut active: taxonomy_category_hierarchy::ActiveModel = existing.into();
                active.parent_term_id = Set(input.parent_id);
                active.position = Set(input.position);
                active.update(&txn).await?
            }
            None => taxonomy_category_hierarchy::ActiveModel {
                tenant_id: Set(tenant_id),
                term_id: Set(term_id),
                parent_term_id: Set(input.parent_id),
                position: Set(input.position),
            }
            .insert(&txn)
            .await?,
        };

        txn.commit().await?;
        Ok(placement_from_model(row))
    }
}

async fn load_category<C>(
    db: &C,
    tenant_id: Uuid,
    term_id: Uuid,
) -> TaxonomyResult<taxonomy_term::Model>
where
    C: sea_orm::ConnectionTrait,
{
    let term = taxonomy_term::Entity::find_by_id(term_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or(TaxonomyError::TermNotFound(term_id))?;
    if term.kind != TaxonomyTermKind::Category {
        return Err(TaxonomyError::validation(format!(
            "Taxonomy term {term_id} is not a Category",
        )));
    }
    Ok(term)
}

fn placement_from_model(row: taxonomy_category_hierarchy::Model) -> TaxonomyCategoryPlacement {
    TaxonomyCategoryPlacement {
        term_id: row.term_id,
        parent_id: row.parent_term_id,
        position: row.position,
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

fn enforce_taxonomy_scope(security: &SecurityContext, action: Action) -> TaxonomyResult<()> {
    if matches!(
        security.get_scope(Resource::Taxonomy, action),
        PermissionScope::None
    ) {
        return Err(TaxonomyError::forbidden("Permission denied"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(term_id: Uuid, parent_term_id: Option<Uuid>) -> taxonomy_category_hierarchy::Model {
        taxonomy_category_hierarchy::Model {
            tenant_id: Uuid::nil(),
            term_id,
            parent_term_id,
            position: 0,
        }
    }

    #[test]
    fn candidate_hierarchy_rejects_cycle() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let rows = vec![row(a, None), row(b, Some(a))];

        let err = validate_candidate_hierarchy(a, Some(b), &rows)
            .expect_err("moving root below its descendant must fail");
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn candidate_hierarchy_rejects_depth_above_platform_limit() {
        let ids = (0..=MAX_TAXONOMY_CATEGORY_DEPTH)
            .map(|_| Uuid::new_v4())
            .collect::<Vec<_>>();
        let rows = ids
            .iter()
            .enumerate()
            .map(|(index, id)| {
                row(
                    *id,
                    if index == 0 { None } else { Some(ids[index - 1]) },
                )
            })
            .collect::<Vec<_>>();
        let extra = Uuid::new_v4();

        let err = validate_candidate_hierarchy(extra, ids.last().copied(), &rows)
            .expect_err("depth above the shared limit must fail");
        assert!(err.to_string().contains("depth 16"));
    }
}
