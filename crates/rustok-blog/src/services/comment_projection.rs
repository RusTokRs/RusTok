use async_trait::async_trait;
use chrono::Utc;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use std::sync::Arc;

use crate::entities::{blog_comment_projection_delivery, blog_post};

const BLOG_POST_TARGET_TYPE: &str = "blog_post";
const FALLBACK_LOCALE: &str = "en";

/// Projects Comments lifecycle events into Blog-owned reply-count state.
///
/// The delivery row, counter update, and BlogPostUpdated outbox record share one
/// transaction. A delivery row is written before the projection work, so retries
/// cannot apply the same event more than once.
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

        let Some(post) = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(envelope.tenant_id))
            .one(&txn)
            .await?
        else {
            txn.commit().await?;
            return Ok(());
        };

        let mut active: blog_post::ActiveModel = post.clone().into();
        active.comment_count = Set((post.comment_count + delta).max(0));
        active.updated_at = Set(Utc::now().into());
        active.version = Set(post.version + 1);
        active.update(&txn).await?;

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
