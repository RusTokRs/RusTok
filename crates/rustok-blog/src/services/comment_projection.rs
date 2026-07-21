use async_trait::async_trait;
use chrono::Utc;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_core::Error;
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};
use std::sync::Arc;

use crate::entities::{blog_comment_projection_delivery, blog_post};

const BLOG_POST_TARGET_TYPE: &str = "blog_post";
const FALLBACK_LOCALE: &str = "en";
const MAX_PROJECTION_UPDATE_ATTEMPTS: usize = 8;

/// Projects Comments lifecycle events into Blog-owned reply-count state.
///
/// The delivery row, counter update, and BlogPostUpdated outbox record share one
/// transaction. Missing Blog posts fail the delivery so the event runtime can
/// retry instead of permanently acknowledging an out-of-order event.
pub struct BlogCommentProjectionHandler {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl BlogCommentProjectionHandler {
    pub fn new(db: DatabaseConnection) -> Self {
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Self { db, event_bus }
    }

    async fn project(&self, envelope: &EventEnvelope) -> HandlerResult {
        let (comment_id, post_id, delta) = match &envelope.event {
            DomainEvent::CommentCreated {
                comment_id,
                target_type,
                target_id,
                ..
            } if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, 1),
            DomainEvent::CommentDeleted {
                comment_id,
                target_type,
                target_id,
                ..
            } if target_type == BLOG_POST_TARGET_TYPE => (*comment_id, *target_id, -1),
            _ => return Ok(()),
        };

        let txn = self.db.begin().await?;
        if blog_comment_projection_delivery::Entity::find_by_id(envelope.id)
            .one(&txn)
            .await?
            .is_some()
        {
            txn.commit().await?;
            return Ok(());
        }

        update_comment_count_in_tx(&txn, envelope.tenant_id, post_id, delta).await?;

        // The delivery marker is committed with the counter and outbox event. If
        // a concurrent duplicate wins this unique insert, this transaction rolls
        // back its optimistic counter update and the runtime can safely retry.
        blog_comment_projection_delivery::ActiveModel {
            event_id: Set(envelope.id),
            tenant_id: Set(envelope.tenant_id),
            comment_id: Set(comment_id),
            post_id: Set(post_id),
            delta: Set(delta),
            processed_at: Set(Utc::now().into()),
        }
        .insert(&txn)
        .await?;

        self.event_bus
            .publish_in_tx(
                &txn,
                envelope.tenant_id,
                envelope.actor_id,
                DomainEvent::BlogPostUpdated {
                    post_id,
                    locale: FALLBACK_LOCALE.to_string(),
                },
            )
            .await?;
        txn.commit().await?;
        Ok(())
    }
}

async fn update_comment_count_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: uuid::Uuid,
    post_id: uuid::Uuid,
    delta: i32,
) -> HandlerResult {
    for _ in 0..MAX_PROJECTION_UPDATE_ATTEMPTS {
        let Some(post) = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
        else {
            return Err(Error::NotFound(format!(
                "blog post {post_id} for comment projection was not found in tenant {tenant_id}"
            )));
        };

        let next_comment_count = post.comment_count.saturating_add(delta).max(0);
        let next_version = post.version.saturating_add(1);
        let result = blog_post::Entity::update_many()
            .col_expr(
                blog_post::Column::CommentCount,
                Expr::value(next_comment_count),
            )
            .col_expr(
                blog_post::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .col_expr(blog_post::Column::Version, Expr::value(next_version))
            .filter(blog_post::Column::Id.eq(post_id))
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .filter(blog_post::Column::Version.eq(post.version))
            .exec(txn)
            .await?;

        if result.rows_affected == 1 {
            return Ok(());
        }
    }

    Err(Error::External(format!(
        "blog comment projection could not update post {post_id} after {MAX_PROJECTION_UPDATE_ATTEMPTS} concurrent attempts"
    )))
}

#[async_trait]
impl EventHandler for BlogCommentProjectionHandler {
    fn name(&self) -> &'static str {
        "blog_comment_projection"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::CommentCreated { target_type, .. }
                | DomainEvent::CommentDeleted { target_type, .. }
                if target_type == BLOG_POST_TARGET_TYPE
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        self.project(envelope).await
    }
}
