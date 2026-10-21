use async_trait::async_trait;
use chrono::Utc;
use rustok_core::Error;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::{blog_comment_projection_delivery, blog_post};
use crate::services::BLOG_POST_TARGET_TYPE;

/// Whether the comment counts toward the public `comment_count` after this
/// event. `None` means "keep the previously projected state" (a body edit does
/// not change visibility). Only `approved` comments are publicly visible
/// (`CommentStatus::is_visible`), so only they may be counted: Blog creates
/// comments as `pending`, and a status change into `spam`/`trash` must remove
/// the comment from the count rather than keep or add it. The moderation queue
/// total is a separate, owner-side number and is not what this counter means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CommentProjectionChange {
    comment_id: Uuid,
    post_id: Uuid,
    counted: Option<bool>,
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
            // Blog comment creation is moderation-pending, so a created
            // comment is not yet part of the public count; approval arrives
            // as a status change.
            counted: Some(false),
        }),
        DomainEvent::CommentUpdated {
            comment_id,
            target_type,
            target_id,
            ..
        } if target_type == BLOG_POST_TARGET_TYPE => Some(CommentProjectionChange {
            comment_id: *comment_id,
            post_id: *target_id,
            counted: None,
        }),
        DomainEvent::CommentStatusChanged {
            comment_id,
            target_type,
            target_id,
            new_status,
            ..
        } if target_type == BLOG_POST_TARGET_TYPE => Some(CommentProjectionChange {
            comment_id: *comment_id,
            post_id: *target_id,
            counted: Some(new_status == "approved"),
        }),
        DomainEvent::CommentDeleted {
            comment_id,
            target_type,
            target_id,
            ..
        } if target_type == BLOG_POST_TARGET_TYPE => Some(CommentProjectionChange {
            comment_id: *comment_id,
            post_id: *target_id,
            counted: Some(false),
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
            // Blog post deletion is terminal and the Comments owner may deliver a
            // previously committed lifecycle event after the post-delete transaction
            // has already committed. The event is obsolete rather than a transient
            // invariant failure; otherwise a valid delete could poison the delivery
            // retry loop forever. Drop stale Blog-owned projection state as part of
            // acknowledging the obsolete event.
            blog_comment_projection_delivery::Entity::delete_many()
                .filter(blog_comment_projection_delivery::Column::TenantId.eq(envelope.tenant_id))
                .filter(blog_comment_projection_delivery::Column::PostId.eq(change.post_id))
                .exec(&txn)
                .await?;
            txn.commit().await?;
            tracing::debug!(
                tenant_id = %envelope.tenant_id,
                post_id = %change.post_id,
                comment_id = %change.comment_id,
                "ignoring Blog comment projection event for deleted post"
            );
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

        // Event id ordering remains a per-comment lifecycle invariant. The snapshot cursor
        // must instead advance on every successfully committed event for the post, even when
        // different comments are processed out of order. The post row lock above serializes
        // this revision allocation without touching Blog business version.
        let latest_projection_revision = blog_comment_projection_delivery::Entity::find()
            .filter(blog_comment_projection_delivery::Column::TenantId.eq(envelope.tenant_id))
            .filter(blog_comment_projection_delivery::Column::PostId.eq(change.post_id))
            .order_by_desc(blog_comment_projection_delivery::Column::ProjectionRevision)
            .one(&txn)
            .await?
            .map(|delivery| delivery.projection_revision)
            .unwrap_or(0);
        let next_projection_revision =
            latest_projection_revision.checked_add(1).ok_or_else(|| {
                Error::Internal(format!(
                    "blog comment projection revision exhausted for post {}",
                    change.post_id
                ))
            })?;

        let latest_delta = latest.as_ref().map(|delivery| delivery.delta);
        let (applied_delta, counted) = projection_applied_delta(latest_delta, change.counted);

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
            return Err(Error::Internal(format!(
                "blog comment projection could not update post {} after row lock",
                change.post_id
            )));
        }

        // Persist the newest per-comment lifecycle state and monotonic post cursor only after the
        // derived counter has succeeded. The same post row lock serializes competing comment events
        // for this post; duplicate delivery remains protected by the event-id primary key.
        blog_comment_projection_delivery::Entity::insert(
            blog_comment_projection_delivery::ActiveModel {
                event_id: Set(envelope.id),
                tenant_id: Set(envelope.tenant_id),
                comment_id: Set(change.comment_id),
                post_id: Set(change.post_id),
                projection_revision: Set(next_projection_revision),
                // Persist the resulting counted state (1 = counts toward the public
                // counter, 0 = does not). Legacy rows written by the previous
                // encoding used +1 for any non-deleted event and -1 for a delete;
                // `projection_applied_delta` reads any positive value as counted,
                // so pre-fix rows keep their meaning until the next event for the
                // comment rewrites them in the new encoding.
                delta: Set(i32::from(counted)),
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

/// Returns `(applied_delta, resulting_counted)`. A stored positive delta means
/// "currently counted" in both the legacy (+1/-1) and the current (1/0)
/// encoding. `target_counted = None` (a body edit) preserves the prior state,
/// so editing a deleted or still-pending comment never changes the counter.
fn projection_applied_delta(
    previous_delta: Option<i32>,
    target_counted: Option<bool>,
) -> (i32, bool) {
    let previous_counted = previous_delta.is_some_and(|delta| delta > 0);
    let counted = target_counted.unwrap_or(previous_counted);
    ((counted as i32) - (previous_counted as i32), counted)
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
        let updated = DomainEvent::CommentUpdated {
            comment_id: id(4),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(5),
            author_id: id(6),
        };
        let status_changed = DomainEvent::CommentStatusChanged {
            comment_id: id(7),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(8),
            author_id: id(9),
            old_status: "pending".to_string(),
            new_status: "approved".to_string(),
        };
        let created = DomainEvent::CommentCreated {
            comment_id: id(1),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(2),
            author_id: id(3),
        };
        let deleted = DomainEvent::CommentDeleted {
            comment_id: id(10),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(11),
            author_id: id(12),
        };

        assert_eq!(
            comment_projection_change(&created),
            Some(CommentProjectionChange {
                comment_id: id(1),
                post_id: id(2),
                counted: Some(false),
            })
        );
        assert_eq!(
            comment_projection_change(&updated),
            Some(CommentProjectionChange {
                comment_id: id(4),
                post_id: id(5),
                counted: None,
            })
        );
        assert_eq!(
            comment_projection_change(&status_changed),
            Some(CommentProjectionChange {
                comment_id: id(7),
                post_id: id(8),
                counted: Some(true),
            })
        );
        assert_eq!(
            comment_projection_change(&deleted),
            Some(CommentProjectionChange {
                comment_id: id(10),
                post_id: id(11),
                counted: Some(false),
            })
        );

        // A status change into a non-public status must un-count the comment.
        let trashed = DomainEvent::CommentStatusChanged {
            comment_id: id(7),
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: id(8),
            author_id: id(9),
            old_status: "approved".to_string(),
            new_status: "trash".to_string(),
        };
        assert_eq!(
            comment_projection_change(&trashed),
            Some(CommentProjectionChange {
                comment_id: id(7),
                post_id: id(8),
                counted: Some(false),
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
        // Creation is pending: not counted. Approval counts it exactly once.
        assert_eq!(projection_applied_delta(None, Some(false)), (0, false));
        assert_eq!(projection_applied_delta(Some(0), Some(true)), (1, true));
        assert_eq!(projection_applied_delta(Some(1), Some(true)), (0, true));
        // Moving an approved comment to spam/trash or deleting it un-counts it.
        assert_eq!(projection_applied_delta(Some(1), Some(false)), (-1, false));
        // A body edit preserves the previous counted state in both directions:
        // it neither resurrects a deleted comment nor hides an approved one.
        assert_eq!(projection_applied_delta(Some(1), None), (0, true));
        assert_eq!(projection_applied_delta(Some(0), None), (0, false));
        assert_eq!(projection_applied_delta(Some(-1), None), (0, false));
        assert_eq!(projection_applied_delta(None, None), (0, false));
    }

    #[test]
    fn counter_transition_is_non_negative_and_does_not_touch_business_revision() {
        assert_eq!(next_comment_count(4, 1), 5);
        assert_eq!(next_comment_count(0, -1), 0);
        assert_eq!(next_comment_count(i32::MAX, 1), i32::MAX);
    }
}
