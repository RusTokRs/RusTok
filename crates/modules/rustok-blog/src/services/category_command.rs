use std::collections::{HashMap, HashSet};

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_taxonomy::entities::taxonomy_category_hierarchy;

use crate::dto::{
    CategoryPlacementResponse, MAX_BLOG_CATEGORY_TREE_NODES, MoveCategoryInput,
    MoveCategoryResponse,
};
use crate::entities::blog_category;
use crate::error::{BlogError, BlogResult};
use crate::services::rbac::enforce_scope;

/// Owner-side structural commands for the Blog category hierarchy.
///
/// Localized copy stays in `CategoryService`; parent/child placement is a separate
/// command so `null` can unambiguously mean "move to root" and hierarchy changes
/// cannot be confused with locale updates. Canonical hierarchy is stored in Taxonomy.
pub struct CategoryCommandService {
    db: DatabaseConnection,
}

impl CategoryCommandService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn move_category(
        &self,
        tenant_id: Uuid,
        category_id: Uuid,
        security: SecurityContext,
        input: MoveCategoryInput,
    ) -> BlogResult<MoveCategoryResponse> {
        enforce_scope(&security, Resource::BlogCategories, Action::Manage)?;

        let txn = self.db.begin().await?;
        lock_category_tree_in_tx(&txn, tenant_id).await?;

        let categories = load_categories_in_tx(&txn, tenant_id).await?;
        let blog_ids = categories
            .iter()
            .map(|category| category.id)
            .collect::<HashSet<_>>();
        if !blog_ids.contains(&category_id) {
            return Err(BlogError::category_not_found(category_id));
        }
        ensure_parent_exists(&blog_ids, input.parent_id)?;

        let hierarchy_rows = taxonomy_category_hierarchy::Entity::find()
            .filter(taxonomy_category_hierarchy::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_category_hierarchy::Column::TermId.is_in(blog_ids.iter().copied()))
            .all(&txn)
            .await?;
        let mut placement_by_id = hierarchy_rows
            .into_iter()
            .map(|row| (row.term_id, (row.parent_term_id, row.position)))
            .collect::<HashMap<_, _>>();
        for id in &blog_ids {
            placement_by_id.entry(*id).or_insert((None, 0));
        }

        let mut parent_by_id = placement_by_id
            .iter()
            .map(|(id, (parent, _))| (*id, *parent))
            .collect::<HashMap<_, _>>();
        let old_depths = validate_and_compute_depths(&parent_by_id)
            .map_err(storage_category_tree_error)?;
        parent_by_id.insert(category_id, input.parent_id);
        let desired_depths = validate_and_compute_depths(&parent_by_id)?;

        let source_parent_id = placement_by_id[&category_id].0;
        let target_index = input.position as usize;
        let mut updated = Vec::new();
        let mut touched = HashSet::new();

        if source_parent_id == input.parent_id {
            let mut siblings = sibling_ids(
                &blog_ids,
                &placement_by_id,
                source_parent_id,
                Some(category_id),
            );
            if target_index > siblings.len() {
                return Err(BlogError::validation(format!(
                    "Category position {} exceeds sibling count {}",
                    input.position,
                    siblings.len()
                )));
            }
            siblings.insert(target_index, category_id);
            updated.extend(
                persist_sibling_order(
                    &txn,
                    tenant_id,
                    &desired_depths,
                    source_parent_id,
                    &siblings,
                    &mut touched,
                )
                .await?,
            );
        } else {
            let source_siblings = sibling_ids(
                &blog_ids,
                &placement_by_id,
                source_parent_id,
                Some(category_id),
            );
            let mut target_siblings =
                sibling_ids(&blog_ids, &placement_by_id, input.parent_id, None);
            if target_index > target_siblings.len() {
                return Err(BlogError::validation(format!(
                    "Category position {} exceeds destination sibling count {}",
                    input.position,
                    target_siblings.len()
                )));
            }
            target_siblings.insert(target_index, category_id);

            updated.extend(
                persist_sibling_order(
                    &txn,
                    tenant_id,
                    &desired_depths,
                    source_parent_id,
                    &source_siblings,
                    &mut touched,
                )
                .await?,
            );
            updated.extend(
                persist_sibling_order(
                    &txn,
                    tenant_id,
                    &desired_depths,
                    input.parent_id,
                    &target_siblings,
                    &mut touched,
                )
                .await?,
            );
        }

        updated.extend(persist_descendant_depth_changes(
            &blog_ids,
            &placement_by_id,
            &old_depths,
            &desired_depths,
            &touched,
        )?);

        let moved = updated
            .iter()
            .find(|placement| placement.id == category_id)
            .cloned()
            .ok_or_else(|| BlogError::invariant("Moved category placement was not persisted"))?;

        txn.commit().await?;
        Ok(MoveCategoryResponse { moved, updated })
    }
}

async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> BlogResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [format!("blog-category-tree:{tenant_id}").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(BlogError::invariant(format!(
            "Blog category hierarchy commands do not support storage backend {backend:?}"
        ))),
    }
}

async fn load_categories_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> BlogResult<Vec<blog_category::Model>> {
    let categories = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(blog_category::Column::Id)
        .limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
        return Err(BlogError::invariant(format!(
            "Persisted Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
        )));
    }
    Ok(categories)
}

fn ensure_parent_exists(blog_ids: &HashSet<Uuid>, parent_id: Option<Uuid>) -> BlogResult<()> {
    if let Some(parent_id) = parent_id
        && !blog_ids.contains(&parent_id)
    {
        return Err(BlogError::validation(format!(
            "Category parent {parent_id} does not exist in the tenant"
        )));
    }
    Ok(())
}

fn sibling_ids(
    blog_ids: &HashSet<Uuid>,
    placement_by_id: &HashMap<Uuid, (Option<Uuid>, i32)>,
    parent_id: Option<Uuid>,
    excluded_id: Option<Uuid>,
) -> Vec<Uuid> {
    let mut siblings = blog_ids
        .iter()
        .copied()
        .filter(|id| {
            let (parent, _) = placement_by_id[id];
            parent == parent_id && Some(*id) != excluded_id
        })
        .map(|id| (id, placement_by_id[&id].1))
        .collect::<Vec<_>>();
    siblings.sort_by(|(left_id, left_pos), (right_id, right_pos)| {
        left_pos.cmp(right_pos).then_with(|| left_id.cmp(right_id))
    });
    siblings.into_iter().map(|(id, _)| id).collect()
}

async fn persist_sibling_order(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    desired_depths: &HashMap<Uuid, i32>,
    parent_id: Option<Uuid>,
    ordered_ids: &[Uuid],
    touched: &mut HashSet<Uuid>,
) -> BlogResult<Vec<CategoryPlacementResponse>> {
    let mut placements = Vec::with_capacity(ordered_ids.len());
    for (position, category_id) in ordered_ids.iter().copied().enumerate() {
        let position = i32::try_from(position)
            .map_err(|_| BlogError::validation("Category sibling position exceeds i32 range"))?;
        let depth = *desired_depths.get(&category_id).ok_or_else(|| {
            BlogError::invariant(format!(
                "Blog category depth was not computed for category {category_id}"
            ))
        })?;

        let existing = taxonomy_category_hierarchy::Entity::find_by_id((tenant_id, category_id))
            .one(txn)
            .await?;
        match existing {
            Some(existing) => {
                if existing.parent_term_id != parent_id || existing.position != position {
                    let mut active: taxonomy_category_hierarchy::ActiveModel = existing.into();
                    active.parent_term_id = Set(parent_id);
                    active.position = Set(position);
                    active.update(txn).await?;
                }
            }
            None => {
                taxonomy_category_hierarchy::ActiveModel {
                    tenant_id: Set(tenant_id),
                    term_id: Set(category_id),
                    parent_term_id: Set(parent_id),
                    position: Set(position),
                }
                .insert(txn)
                .await?;
            }
        }

        touched.insert(category_id);
        placements.push(CategoryPlacementResponse {
            id: category_id,
            parent_id,
            position,
            depth,
        });
    }
    Ok(placements)
}

fn persist_descendant_depth_changes(
    blog_ids: &HashSet<Uuid>,
    placement_by_id: &HashMap<Uuid, (Option<Uuid>, i32)>,
    old_depths: &HashMap<Uuid, i32>,
    desired_depths: &HashMap<Uuid, i32>,
    touched: &HashSet<Uuid>,
) -> BlogResult<Vec<CategoryPlacementResponse>> {
    let mut placements = Vec::new();
    let mut category_ids = blog_ids.iter().copied().collect::<Vec<_>>();
    category_ids.sort();

    for category_id in category_ids {
        if touched.contains(&category_id) {
            continue;
        }
        let (parent_id, position) = placement_by_id[&category_id];
        let old_depth = old_depths.get(&category_id).copied().ok_or_else(|| {
            BlogError::invariant(format!(
                "Persisted Blog category depth is missing for category {category_id}"
            ))
        })?;
        let desired_depth = desired_depths.get(&category_id).copied().ok_or_else(|| {
            BlogError::invariant(format!(
                "Desired Blog category depth is missing for category {category_id}"
            ))
        })?;
        if old_depth != desired_depth {
            placements.push(CategoryPlacementResponse {
                id: category_id,
                parent_id,
                position,
                depth: desired_depth,
            });
        }
    }
    Ok(placements)
}

fn storage_category_tree_error(error: BlogError) -> BlogError {
    match error {
        BlogError::Validation(message) => BlogError::invariant(format!(
            "Persisted Blog category hierarchy is invalid: {message}"
        )),
        other => other,
    }
}

fn validate_and_compute_depths(
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
) -> BlogResult<HashMap<Uuid, i32>> {
    if parent_by_id.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
        return Err(BlogError::validation(format!(
            "Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
        )));
    }

    let mut depths = HashMap::with_capacity(parent_by_id.len());
    for category_id in parent_by_id.keys().copied() {
        let mut active_path = HashSet::new();
        compute_depth(category_id, parent_by_id, &mut depths, &mut active_path)?;
    }
    Ok(depths)
}

fn compute_depth(
    category_id: Uuid,
    parent_by_id: &HashMap<Uuid, Option<Uuid>>,
    depths: &mut HashMap<Uuid, i32>,
    active_path: &mut HashSet<Uuid>,
) -> BlogResult<i32> {
    if let Some(depth) = depths.get(&category_id) {
        return Ok(*depth);
    }
    if !active_path.insert(category_id) {
        return Err(BlogError::validation("Blog category hierarchy cycle"));
    }

    let parent_id = parent_by_id.get(&category_id).ok_or_else(|| {
        BlogError::validation(format!(
            "Blog category tree references missing category {category_id}"
        ))
    })?;
    let depth = match *parent_id {
        None => 0,
        Some(parent_id) => {
            if !parent_by_id.contains_key(&parent_id) {
                return Err(BlogError::validation(format!(
                    "Blog category tree references missing or foreign parent {parent_id}"
                )));
            }
            compute_depth(parent_id, parent_by_id, depths, active_path)?
                .checked_add(1)
                .ok_or_else(|| {
                    BlogError::validation("Blog category hierarchy depth is exhausted")
                })?
        }
    };

    active_path.remove(&category_id);
    depths.insert(category_id, depth);
    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_depths_and_rejects_cycles() {
        let root = Uuid::from_u128(1);
        let child = Uuid::from_u128(2);
        let grandchild = Uuid::from_u128(3);
        let tree = HashMap::from([(root, None), (child, Some(root)), (grandchild, Some(child))]);
        let depths = validate_and_compute_depths(&tree).expect("valid Blog tree");
        assert_eq!(depths[&root], 0);
        assert_eq!(depths[&child], 1);
        assert_eq!(depths[&grandchild], 2);

        let cycle = HashMap::from([(root, Some(child)), (child, Some(root))]);
        assert!(validate_and_compute_depths(&cycle).is_err());
    }

    #[test]
    fn persisted_hierarchy_validation_is_reclassified_as_invariant() {
        let error = storage_category_tree_error(BlogError::validation(
            "Blog category hierarchy cycle",
        ));
        assert!(matches!(error, BlogError::Invariant(_)));
    }

    #[test]
    fn rejects_missing_parent_and_unbounded_tree() {
        let root = Uuid::from_u128(1);
        let missing = Uuid::from_u128(99);
        assert!(validate_and_compute_depths(&HashMap::from([(root, Some(missing))])).is_err());

        let oversized = (0..=MAX_BLOG_CATEGORY_TREE_NODES)
            .map(|index| (Uuid::from_u128(index as u128 + 1), None))
            .collect::<HashMap<_, _>>();
        assert!(validate_and_compute_depths(&oversized).is_err());
    }
}
