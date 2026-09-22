use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::{
    TaxonomyCategoryDeleteCleanupPort, TaxonomyError, TaxonomyResult,
};
use sea_orm::{
    ColumnTrait, DatabaseTransaction, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
};
use uuid::Uuid;

use crate::entities::{blog_category, blog_post};
use crate::{BlogError, BlogResult};

/// Blog-owned cleanup that participates in Taxonomy's canonical Category delete transaction.
///
/// The outer Taxonomy owner deletes the canonical term only after this cleanup succeeds. Blog
/// removes its membership row, compacts and replays sibling placement, publishes reindex evidence
/// and finally delegates to the host-owned capability cleanup (Flex in the server composition).
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
        rustok_taxonomy::lock_category_hierarchy_writer_in_tx(txn, tenant_id).await?;

        if self.blog_category_id != taxonomy_category_id {
            return Err(BlogError::invariant(format!(
                "Blog category {} does not match delete target {}",
                self.blog_category_id, taxonomy_category_id
            )));
        }

        let category = blog_category::Entity::find_by_id(self.blog_category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(txn)
            .await?
            .ok_or_else(|| BlogError::category_not_found(self.blog_category_id))?;
        rustok_taxonomy::delete_module_category_placement_and_compact_in_tx(
            txn,
            tenant_id,
            self.blog_category_id,
            crate::services::category_taxonomy_sync::BLOG_TAXONOMY_SCOPE,
        )
        .await?;

        detach_category_from_posts_in_tx(txn, tenant_id, self.blog_category_id).await?;

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

async fn detach_category_from_posts_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    let now = Utc::now();

    blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::CategoryId,
            sea_orm::sea_query::Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            blog_post::Column::Version,
            sea_orm::sea_query::Expr::col(blog_post::Column::Version).add(1),
        )
        .col_expr(
            blog_post::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::CategoryId.eq(category_id))
        .filter(blog_post::Column::Version.gt(0))
        .filter(blog_post::Column::Version.ne(i64::MAX))
        .exec(txn)
        .await?;

    let remaining = blog_post::Entity::find()
        .filter(blog_post::Column::TenantId.eq(tenant_id))
        .filter(blog_post::Column::CategoryId.eq(category_id))
        .count(txn)
        .await?;

    if remaining != 0 {
        return Err(BlogError::invariant(format!(
            "Blog category {category_id} cannot be detached from {remaining} post(s) because their persisted version is invalid or exhausted",
        )));
    }

    Ok(())
}

fn map_blog_error(error: BlogError) -> TaxonomyError {
    match error {
        BlogError::Database(error) => TaxonomyError::Database(error),
        BlogError::CategoryNotFound(category_id) => TaxonomyError::TermNotFound(category_id),
        BlogError::Conflict(message) => TaxonomyError::conflict(message),
        BlogError::Forbidden(message) => TaxonomyError::forbidden(message),
        BlogError::Validation(message) => TaxonomyError::validation(message),
        other => TaxonomyError::internal(format!(
            "Blog Category delete cleanup failed: {other:?}"
        )),
    }
}
