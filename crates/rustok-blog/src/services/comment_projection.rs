use async_trait::async_trait;
use chrono::Utc;
use rustok_core::Error;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait, sea_query::Expr,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::{blog_comment_projection_delivery, blog_post};

const BLOG_POST_TARGET_TYPE: &str = "blog_post";
const FALLBACK_LOCALE: &str = "en";
const MAX_PROJECTION_UPDATE_ATTEMPTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommentProjectionChange {
    comment_id: Uuid,
    post_id: Uuid,
    delta: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProjectionUpdateDecision {
    Applied,
    Retry,
    LimitReached,
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

fn next_comment_projection_state(comment_count: i32, version: i32, delta: i32) -> (i32, i32) {
    (
        comment_count.saturating_add(delta).max(0),
        version.saturating_add(1),
    )
}

fn projection_update_decision(
    attempt_index: usize,
    rows_affected: u64,
) -> ProjectionUpdateDecision {
    if rows_affected == 1 {
        ProjectionUpdateDecision::Applied
    } else if attempt_index + 1 < MAX_PROJECTION_UPDATE_ATTEMPTS {
        ProjectionUpdateDecision::Retry
    } else {
        ProjectionUpdateDecision::LimitReached
    }
}

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
        let Some(change) = comment_projection_change(&envelope.event) else {
            return Ok(());
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

        update_comment_count_in_tx(&txn, envelope.tenant_id, change.post_id, change.delta).await?;

        // The delivery marker is committed with the counter and outbox event. If
        // a concurrent duplicate wins this unique insert, this transaction rolls
        // back its optimistic counter update and the runtime can safely retry.
        blog_comment_projection_delivery::ActiveModel {
            event_id: Set(envelope.id),
            tenant_id: Set(envelope.tenant_id),
            comment_id: Set(change.comment_id),
            post_id: Set(change.post_id),
            delta: Set(change.delta),
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
                    post_id: change.post_id,
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
    tenant_id: Uuid,
    post_id: Uuid,
    delta: i32,
) -> HandlerResult {
    for attempt_index in 0..MAX_PROJECTION_UPDATE_ATTEMPTS {
        let Some(post) = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
        else {
            return Err(Error::NotFound(format!(
                "blog post {post_id} for comment projection was not found in tenant {tenant_id}"
            )));
        };

        let (next_comment_count, next_version) =
            next_comment_projection_state(post.comment_count, post.version, delta);
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

        match projection_update_decision(attempt_index, result.rows_affected) {
            ProjectionUpdateDecision::Applied => return Ok(()),
            ProjectionUpdateDecision::Retry => continue,
            ProjectionUpdateDecision::LimitReached => break,
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
    fn counter_transition_is_non_negative_and_saturating() {
        assert_eq!(next_comment_projection_state(4, 11, 1), (5, 12));
        assert_eq!(next_comment_projection_state(0, 11, -1), (0, 12));
        assert_eq!(
            next_comment_projection_state(i32::MAX, i32::MAX, 1),
            (i32::MAX, i32::MAX)
        );
    }

    #[test]
    fn optimistic_retry_policy_applies_success_without_retry() {
        assert_eq!(
            projection_update_decision(0, 1),
            ProjectionUpdateDecision::Applied
        );
        assert_eq!(
            projection_update_decision(MAX_PROJECTION_UPDATE_ATTEMPTS - 1, 1),
            ProjectionUpdateDecision::Applied
        );
    }

    #[test]
    fn optimistic_retry_policy_allows_seven_retries_then_stops_on_eighth_conflict() {
        let decisions = (0..MAX_PROJECTION_UPDATE_ATTEMPTS)
            .map(|attempt_index| projection_update_decision(attempt_index, 0))
            .collect::<Vec<_>>();

        assert_eq!(
            decisions
                .iter()
                .filter(|decision| **decision == ProjectionUpdateDecision::Retry)
                .count(),
            MAX_PROJECTION_UPDATE_ATTEMPTS - 1
        );
        assert_eq!(
            decisions.last(),
            Some(&ProjectionUpdateDecision::LimitReached)
        );
    }
}
