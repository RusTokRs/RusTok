use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{TaxonomyCategoryDeleteCleanupPort, TaxonomyError, TaxonomyResult};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend,
    DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, Statement,
};
use uuid::Uuid;

use crate::entities::{blog_category, blog_category_taxonomy_binding};
use crate::{BlogError, BlogResult};

use super::category_taxonomy_sync::sync_category_structures_in_tx;

/// Blog-owned cleanup that participates in Taxonomy's canonical Category delete transaction.
///
/// The outer Taxonomy owner deletes the canonical term only after this cleanup succeeds. Blog
/// removes its membership row, compacts and replays sibling placement, publishes reindex evidence
/// and finally delegates to the host-owned capability cleanup (Flex in the server composition).
/// The retired Blog Translation change journal is intentionally not part of this lifecycle.
pub(crate) struct BlogCategoryDeleteCleanup {
    blog_category_id: Uuid,
    actor_id: Option<Uuid>,
    event_bus: TransactionalEventBus,
    capability_cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
}

impl BlogCategoryDeleteCleanup {
    pub(crate) fn new(
        blog_category_id: Uuid,
        actor_id: Option<Uuid>,
        event_bus: TransactionalEventBus,
        capability_cleanup: Arc<dyn TaxonomyCategoryDeleteCleanupPort>,
    ) -> Self {
        Self {
            blog_category_id,
            actor_id,
            event_bus,
            capability_cleanup,
        }
    }

    async fn cleanup_blog_membership_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        taxonomy_category_id: Uuid,
    ) -> BlogResult<()> {
        lock_category_tree_in_tx(txn, tenant_id).await?;

        let binding =
            blog_category_taxonomy_binding::Entity::find_by_id((tenant_id, self.blog_category_id))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    BlogError::validation(format!(
                        "Blog category {} has no Taxonomy Category binding",
                        self.blog_category_id
                    ))
                })?;
        if binding.taxonomy_category_id != taxonomy_category_id {
            return Err(BlogError::validation(format!(
                "Blog category {} is bound to Taxonomy Category {}, not delete target {}",
                self.blog_category_id, binding.taxonomy_category_id, taxonomy_category_id
            )));
        }

        let category = blog_category::Entity::find_by_id(self.blog_category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(self.blog_category_id))?;
        ensure_category_is_leaf_in_tx(txn, tenant_id, self.blog_category_id).await?;

        let deleted = blog_category::Entity::delete_many()
            .filter(blog_category::Column::Id.eq(self.blog_category_id))
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .filter(blog_category::Column::Revision.eq(category.revision))
            .exec(txn)
            .await?;
        if deleted.rows_affected != 1 {
            return Err(BlogError::conflict(
                "blog category changed before deletion could commit",
            ));
        }

        let sibling_ids = canonicalize_siblings_in_tx(txn, tenant_id, category.parent_id).await?;
        sync_category_structures_in_tx(txn, tenant_id, &sibling_ids).await?;

        self.event_bus
            .publish_in_tx(
                txn,
                tenant_id,
                self.actor_id,
                DomainEvent::ReindexRequested {
                    target_type: "blog".to_string(),
                    target_id: None,
                },
            )
            .await
            .map_err(BlogError::from)?;

        Ok(())
    }
}

#[async_trait]
impl TaxonomyCategoryDeleteCleanupPort for BlogCategoryDeleteCleanup {
    async fn cleanup_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> TaxonomyResult<()> {
        self.cleanup_blog_membership_in_tx(txn, tenant_id, category_id)
            .await
            .map_err(map_blog_error)?;
        self.capability_cleanup
            .cleanup_in_tx(txn, tenant_id, category_id)
            .await
    }
}

async fn lock_category_tree_in_tx(txn: &DatabaseTransaction, tenant_id: Uuid) -> BlogResult<()> {
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
            "Blog category hierarchy writes do not support {backend:?}"
        ))),
    }
}

async fn ensure_category_is_leaf_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    let child = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .filter(blog_category::Column::ParentId.eq(category_id))
        .one(txn)
        .await?;
    if child.is_some() {
        return Err(BlogError::validation(
            "Category must be a leaf before deletion; move or delete its children first",
        ));
    }
    Ok(())
}

async fn canonicalize_siblings_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> BlogResult<Vec<Uuid>> {
    let mut query =
        blog_category::Entity::find().filter(blog_category::Column::TenantId.eq(tenant_id));
    query = match parent_id {
        Some(parent_id) => query.filter(blog_category::Column::ParentId.eq(parent_id)),
        None => query.filter(blog_category::Column::ParentId.is_null()),
    };
    let siblings = query
        .order_by_asc(blog_category::Column::Position)
        .order_by_asc(blog_category::Column::Id)
        .all(txn)
        .await?;
    let now = Utc::now();
    let mut sibling_ids = Vec::with_capacity(siblings.len());
    for (index, sibling) in siblings.into_iter().enumerate() {
        let desired_position = i32::try_from(index)
            .map_err(|_| BlogError::validation("Category sibling position exceeds i32 range"))?;
        sibling_ids.push(sibling.id);
        if sibling.position == desired_position {
            continue;
        }
        let mut active: blog_category::ActiveModel = sibling.into();
        active.position = Set(desired_position);
        active.updated_at = Set(now.into());
        active.update(txn).await?;
    }
    Ok(sibling_ids)
}

fn map_blog_error(error: BlogError) -> TaxonomyError {
    match error {
        BlogError::Database(error) => TaxonomyError::Database(error),
        other => TaxonomyError::validation(format!("Blog Category delete cleanup failed: {other}")),
    }
}
