use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    Statement, TransactionTrait,
};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

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
/// cannot be confused with locale updates.
pub struct CategoryCommandService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl CategoryCommandService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
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
        let models = categories
            .iter()
            .cloned()
            .map(|category| (category.id, category))
            .collect::<HashMap<_, _>>();
        let category = models
            .get(&category_id)
            .cloned()
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        ensure_parent_exists(&models, input.parent_id)?;

        let mut parent_by_id = models
            .values()
            .map(|category| (category.id, category.parent_id))
            .collect::<HashMap<_, _>>();
        validate_and_compute_depths(&parent_by_id)?;
        parent_by_id.insert(category_id, input.parent_id);
        let desired_depths = validate_and_compute_depths(&parent_by_id)?;

        let source_parent_id = category.parent_id;
        let target_index = input.position as usize;
        let mut updated = Vec::new();
        let mut touched = HashSet::new();

        if source_parent_id == input.parent_id {
            let mut siblings = sibling_ids(&categories, source_parent_id, Some(category_id));
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
                    &models,
                    &desired_depths,
                    source_parent_id,
                    &siblings,
                    &mut touched,
                )
                .await?,
            );
        } else {
            let source_siblings = sibling_ids(&categories, source_parent_id, Some(category_id));
            let mut target_siblings = sibling_ids(&categories, input.parent_id, None);
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
                    &models,
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
                    &models,
                    &desired_depths,
                    input.parent_id,
                    &target_siblings,
                    &mut touched,
                )
                .await?,
            );
        }

        updated.extend(
            persist_descendant_depth_changes(
                &txn,
                &models,
                &desired_depths,
                &touched,
            )
            .await?,
        );

        let moved = updated
            .iter()
            .find(|placement| placement.id == category_id)
            .cloned()
            .ok_or_else(|| BlogError::validation("Moved category placement was not persisted"))?;

        self.event_bus
            .publish_in_tx(
                &txn,
                tenant_id,
                security.user_id,
                DomainEvent::ReindexRequested {
                    target_type: "blog".to_string(),
                    target_id: None,
                },
            )
            .await
            .map_err(BlogError::from)?;

        txn.commit().await?;
        Ok(MoveCategoryResponse { moved, updated })
    }
}

async fn lock_category_tree_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> BlogResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [format!("blog-category-tree:{tenant_id}").into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(BlogError::validation(format!(
            "Blog category hierarchy commands do not support {backend:?}"
        ))),
    }
}

async fn load_categories_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> BlogResult<Vec<blog_category::Model>> {
    let categories = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(blog_category::Column::Position)
        .order_by_asc(blog_category::Column::Id)
        .limit(MAX_BLOG_CATEGORY_TREE_NODES + 1)
        .all(txn)
        .await?;
    if categories.len() > MAX_BLOG_CATEGORY_TREE_NODES as usize {
        return Err(BlogError::validation(format!(
            "Blog category tree exceeds the bounded limit of {MAX_BLOG_CATEGORY_TREE_NODES} nodes"
        )));
    }
    Ok(categories)
}

fn ensure_parent_exists(
    models: &HashMap<Uuid, blog_category::Model>,
    parent_id: Option<Uuid>,
) -> BlogResult<()> {
    if let Some(parent_id) = parent_id
        && !models.contains_key(&parent_id)
    {
        return Err(BlogError::validation(format!(
            "Category parent {parent_id} does not exist in the tenant"
        )));
    }
    Ok(())
}

fn sibling_ids(
    categories: &[blog_category::Model],
    parent_id: Option<Uuid>,
    excluded_id: Option<Uuid>,
) -> Vec<Uuid> {
    categories
        .iter()
        .filter(|category| category.parent_id == parent_id && Some(category.id) != excluded_id)
        .map(|category| category.id)
        .collect()
}

async fn persist_sibling_order(
    txn: &DatabaseTransaction,
    models: &HashMap<Uuid, blog_category::Model>,
    desired_depths: &HashMap<Uuid, i32>,
    parent_id: Option<Uuid>,
    ordered_ids: &[Uuid],
    touched: &mut HashSet<Uuid>,
) -> BlogResult<Vec<CategoryPlacementResponse>> {
    let mut placements = Vec::with_capacity(ordered_ids.len());
    for (position, category_id) in ordered_ids.iter().copied().enumerate() {
        let category = models
            .get(&category_id)
            .cloned()
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        let position = i32::try_from(position)
            .map_err(|_| BlogError::validation("Category sibling position exceeds i32 range"))?;
        let depth = *desired_depths.get(&category_id).ok_or_else(|| {
            BlogError::validation(format!(
                "Blog category depth was not computed for category {category_id}"
            ))
        })?;

        if category.parent_id != parent_id
            || category.position != position
            || category.depth != depth
        {
            let mut active: blog_category::ActiveModel = category.into();
            active.parent_id = Set(parent_id);
            active.position = Set(position);
            active.depth = Set(depth);
            active.updated_at = Set(Utc::now().into());
            active.update(txn).await?;
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

async fn persist_descendant_depth_changes(
    txn: &DatabaseTransaction,
    models: &HashMap<Uuid, blog_category::Model>,
    desired_depths: &HashMap<Uuid, i32>,
    touched: &HashSet<Uuid>,
) -> BlogResult<Vec<CategoryPlacementResponse>> {
    let mut placements = Vec::new();
    let mut category_ids = models.keys().copied().collect::<Vec<_>>();
    category_ids.sort();

    for category_id in category_ids {
        if touched.contains(&category_id) {
            continue;
        }
        let category = models
            .get(&category_id)
            .cloned()
            .ok_or_else(|| BlogError::category_not_found(category_id))?;
        let depth = *desired_depths.get(&category_id).ok_or_else(|| {
            BlogError::validation(format!(
                "Blog category depth was not computed for category {category_id}"
            ))
        })?;
        if category.depth == depth {
            continue;
        }

        let mut active: blog_category::ActiveModel = category.clone().into();
        active.depth = Set(depth);
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        placements.push(CategoryPlacementResponse {
            id: category_id,
            parent_id: category.parent_id,
            position: category.position,
            depth,
        });
    }
    Ok(placements)
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
        let tree = HashMap::from([
            (root, None),
            (child, Some(root)),
            (grandchild, Some(child)),
        ]);
        let depths = validate_and_compute_depths(&tree).expect("valid Blog tree");
        assert_eq!(depths[&root], 0);
        assert_eq!(depths[&child], 1);
        assert_eq!(depths[&grandchild], 2);

        let cycle = HashMap::from([(root, Some(child)), (child, Some(root))]);
        assert!(validate_and_compute_depths(&cycle).is_err());
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
