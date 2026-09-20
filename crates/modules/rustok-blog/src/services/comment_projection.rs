use async_trait::async_trait;
use chrono::Utc;
use rustok_core::Error;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::{blog_comment_projection_delivery, blog_post};

const BLOG_POST_TARGET_TYPE: &str = "blog_post";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommentProjectionChange {
    comment_id: Uuid,
    post_id: Uuid,
    delta: i32,
}

fn comment_projection_change(event: &DomainEvent) -> Option<CommentProjectionChange> {
    match event {
        DomainEvent::CommentCreated {
            comment_id,
            target_type,
            target_id,
            ..
        } if target_type == BLOG_POST_TARGET_TYPE => Some(CommentProjectionChange {
            comment_id: *comment_id,
            post_id: *target_id,
            delta: 1,
        }),
        DomainEvent::CommentDeleted {
            comment_id,
            target_type,
            target_id,
            ..
        } if target_type == BLOG_POST_TARGET_TYPE => Some(CommentProjectionChange {
            comment_id: *comment_id,
            post_id: *target_id,
            delta: -1,
        }),
        _ => None,
    }
}

fn next_comment_count(comment_count: i32, delta: i32) -> i32 {
    comment_count.saturating_add(delta).max(0)
}

/// Projects Comments lifecycle events into Blog-owned reply-count state.
///
/// The delivery row, derived counter update, and reindex request share one
/// transaction. Derived counters deliberately do not mutate the Blog business
/// revision or content `updated_at`; otherwise an external Comments event could
/// invalidate an editor CAS token or a Reaction subject revision.
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
        let Some(change) = comment_projection_change(&envelope.event) else {
            return Ok(());
        };

        let txn = self.db.begin().await?;
        let Some(post) = blog_post::Entity::find_by_id(change.post_id)
            .filter(blog_post::Column::TenantId.eq(envelope.tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await?
        else {
            txn.commit().await?;
            return Ok(());
        };

        // Event IDs are ULIDs encoded as UUIDs by EventEnvelope::new(), so UUID ordering
        // preserves event creation order. The Blog projection only needs that ordering
        // per comment, not globally: independent comments may legitimately interleave.
        let latest = blog_comment_projection_delivery::Entity::find()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(envelope.tenant_id))
            .filter(blog_comment_projection_delivery::Column::PostId.eq(change.post_id))
            .filter(blog_comment_projection_delivery::Column::CommentId.eq(change.comment_id))
            .order_by_desc(blog_comment_projection_delivery::Column::EventId)
            .one(&txn)
            .await?;

        if latest
            .as_ref()
            .is_some_and(|delivery| delivery.event_id >= envelope.id)
        {
            txn.commit().await?;
            return Ok(());
        }

        let applied_delta = projection_applied_delta(
            latest.as_ref().map(|delivery| delivery.delta),
            change.delta,
        );

        let next_comment_count = next_comment_count(post.comment_count, applied_delta);
        let post_updated = if applied_delta == 0 {
            false
        } else {
            let result = blog_post::Entity::update_many()
                .col_expr(
                    blog_post::Column::CommentCount,
                    Expr::value(next_comment_count),
                )
                .filter(blog_post::Column::Id.eq(post.id))
                .filter(blog_post::Column::TenantId.eq(envelope.tenant_id))
                .filter(blog_post::Column::CommentCount.eq(post.comment_count))
                .exec(&txn)
                .await?;
            result.rows_affected == 1
        };

        if applied_delta != 0 && !post_updated {
            return Err(Error::External(format!(
                "blog comment projection could not update post {} after row lock",
                change.post_id
            )));
        }

        // Persist the newest per-comment lifecycle state only after the derived counter has
        // succeeded. The same post row lock serializes competing comment events for this post;
        // duplicate delivery remains protected by the event-id primary key.
        blog_comment_projection_delivery::Entity::insert(
            blog_comment_projection_delivery::ActiveModel {
                event_id: Set(envelope.id),
                tenant_id: Set(envelope.tenant_id),
                comment_id: Set(change.comment_id),
                post_id: Set(change.post_id),
                // Keep the source event state (+1 created / -1 deleted). For an out-of-order
                // first delete this remains -1 even when the counter delta is necessarily zero.
                delta: Set(change.delta),
                processed_at: Set(Utc::now().into()),
            },
        )
        .on_conflict(
            OnConflict::column(blog_comment_projection_delivery::Column::EventId)
                .do_nothing()
                .to_owned(),
        )
        .exec(&txn)
        .await?;

        if post_updated {
            self.event_bus
                .publish_in_tx(
                    &txn,
                    envelope.tenant_id,
                    envelope.actor_id,
                    DomainEvent::ReindexRequested {
                        target_type: "blog".to_string(),
                        target_id: Some(change.post_id),
                    },
                )
                .await?;
        }
        txn.commit().await?;
        Ok(())
    }
}

fn projection_applied_delta(previous_delta: Option<i32>, current_delta: i32) -> i32 {
    let previous_active = previous_delta.is_some_and(|delta| delta > 0);
    let current_active = current_delta > 0;
    (current_active as i32) - (previous_active as i32)
}

#[async_trait]
impl EventHandler for BlogCommentProjectionHandler {
    fn name(&self) -> &'static str {
        "blog_comment_projection"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        comment_projection_change(event).is_some()
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        self.project(envelope).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    #[test]
    fn classifies_blog_comment_lifecycle_events() {
        let created = DomainEvent::CommentCreated {
            comment_id: id(1),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(2),
            author_id: id(3),
        };
        let deleted = DomainEvent::CommentDeleted {
            comment_id: id(4),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(5),
            author_id: id(6),
        };

        assert_eq!(
            comment_projection_change(&created),
            Some(CommentProjectionChange {
                comment_id: id(1),
                post_id: id(2),
                delta: 1,
            })
        );
        assert_eq!(
            comment_projection_change(&deleted),
            Some(CommentProjectionChange {
                comment_id: id(4),
                post_id: id(5),
                delta: -1,
            })
        );
    }

    #[test]
    fn ignores_non_blog_targets_and_unrelated_events() {
        let other_target = DomainEvent::CommentCreated {
            comment_id: id(7),
            target_type: "forum_topic".to_string(),
            target_id: id(8),
            author_id: id(9),
        };
        let unrelated = DomainEvent::BlogPostUpdated {
            post_id: id(10),
            locale: "en".to_string(),
        };

        assert_eq!(comment_projection_change(&other_target), None);
        assert_eq!(comment_projection_change(&unrelated), None);
    }

    #[test]
    fn projection_delta_tracks_comment_state_not_delivery_order() {
        assert_eq!(projection_applied_delta(None, 1), 1);
        assert_eq!(projection_applied_delta(None, -1), 0);
        assert_eq!(projection_applied_delta(Some(1), -1), -1);
        assert_eq!(projection_applied_delta(Some(-1), 1), 1);
    }

    #[test]
    fn counter_transition_is_non_negative_and_does_not_touch_business_revision() {
        assert_eq!(next_comment_count(4, 1), 5);
        assert_eq!(next_comment_count(0, -1), 0);
        assert_eq!(next_comment_count(i32::MAX, 1), i32::MAX);
    }
}
