//! Per-author posting rate limits for new topics and replies.
//!
//! The check compares the author's most recent topic (or reply) in the tenant with the
//! current time. It runs inside the write transaction, after the tenant-wide category-tree
//! lock that topic and reply creation already take. That lock serialises every Forum create
//! in the tenant, so two concurrent creates by the same author cannot both pass the check.
//!
//! Only user actors are rate limited. System and service actors are internal writers. See
//! `DECISIONS/2026-10-09-forum-posting-rate-limits.md`.

use chrono::{DateTime, FixedOffset, Utc};
use rustok_core::{SecurityActorKind, SecurityContext};
use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::entities::{forum_reply, forum_topic};
use crate::error::{ForumError, ForumResult};

pub(crate) async fn enforce_new_topic_rate_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    security: &SecurityContext,
    limit_seconds: u32,
) -> ForumResult<()> {
    let Some(author_id) = rate_limited_author(security, limit_seconds)? else {
        return Ok(());
    };
    let last_created_at = forum_topic::Entity::find()
        .filter(forum_topic::Column::TenantId.eq(tenant_id))
        .filter(forum_topic::Column::AuthorId.eq(author_id))
        .order_by_desc(forum_topic::Column::CreatedAt)
        .one(txn)
        .await?
        .map(|topic| topic.created_at);
    reject_if_within_interval(last_created_at, limit_seconds, Utc::now())
}

pub(crate) async fn enforce_new_reply_rate_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    security: &SecurityContext,
    limit_seconds: u32,
) -> ForumResult<()> {
    let Some(author_id) = rate_limited_author(security, limit_seconds)? else {
        return Ok(());
    };
    let last_created_at = forum_reply::Entity::find()
        .filter(forum_reply::Column::TenantId.eq(tenant_id))
        .filter(forum_reply::Column::AuthorId.eq(author_id))
        .order_by_desc(forum_reply::Column::CreatedAt)
        .one(txn)
        .await?
        .map(|reply| reply.created_at);
    reject_if_within_interval(last_created_at, limit_seconds, Utc::now())
}

/// Only human users posting through their own session are rate limited. System and service
/// actors are trusted internal writers, even when they record a user id for attribution. A user
/// actor without an id cannot be attributed to an author, so it is rejected rather than exempted.
fn rate_limited_author(
    security: &SecurityContext,
    limit_seconds: u32,
) -> ForumResult<Option<Uuid>> {
    if limit_seconds == 0 || security.actor_kind != SecurityActorKind::User {
        return Ok(None);
    }
    security
        .user_id
        .map(Some)
        .ok_or_else(|| ForumError::forbidden("Forum posting requires an authenticated author"))
}

fn reject_if_within_interval(
    last_created_at: Option<DateTime<FixedOffset>>,
    limit_seconds: u32,
    now: DateTime<Utc>,
) -> ForumResult<()> {
    match retry_after_seconds(last_created_at, limit_seconds, now) {
        Some(retry_after_seconds) => Err(ForumError::RateLimited {
            retry_after_seconds,
        }),
        None => Ok(()),
    }
}

/// Returns the seconds to wait when the previous post is newer than the interval, otherwise `None`.
/// A previous post timestamp in the future counts as zero elapsed seconds, so clock skew
/// fails closed.
fn retry_after_seconds(
    last_created_at: Option<DateTime<FixedOffset>>,
    limit_seconds: u32,
    now: DateTime<Utc>,
) -> Option<u64> {
    let last_created_at = last_created_at?;
    let limit = u64::from(limit_seconds);
    let elapsed =
        u64::try_from(now.signed_duration_since(last_created_at).num_seconds()).unwrap_or(0);
    (elapsed < limit).then(|| limit - elapsed)
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, FixedOffset, Utc};
    use rustok_core::{SecurityContext, UserRole};
    use uuid::Uuid;

    use super::{rate_limited_author, retry_after_seconds};

    #[test]
    fn no_previous_post_is_never_limited() {
        assert_eq!(retry_after_seconds(None, 10, Utc::now()), None);
    }

    #[test]
    fn post_inside_the_interval_reports_the_remaining_wait() {
        let now = Utc::now();
        let last = (now - Duration::seconds(3)).with_timezone(&FixedOffset::east_opt(0).unwrap());
        assert_eq!(retry_after_seconds(Some(last), 10, now), Some(7));
    }

    #[test]
    fn post_at_or_after_the_interval_is_allowed() {
        let now = Utc::now();
        let last = (now - Duration::seconds(10)).with_timezone(&FixedOffset::east_opt(0).unwrap());
        assert_eq!(retry_after_seconds(Some(last), 10, now), None);
    }

    #[test]
    fn future_timestamp_fails_closed_for_the_full_interval() {
        let now = Utc::now();
        let last = (now + Duration::seconds(60)).with_timezone(&FixedOffset::east_opt(0).unwrap());
        assert_eq!(retry_after_seconds(Some(last), 10, now), Some(10));
    }

    #[test]
    fn system_actor_is_not_limited_even_when_it_records_an_author() {
        let mut security = SecurityContext::system();
        security.user_id = Some(Uuid::new_v4());
        assert!(matches!(rate_limited_author(&security, 10), Ok(None)));
    }

    #[test]
    fn user_actor_is_limited_by_its_user_id() {
        let user_id = Uuid::new_v4();
        let security = SecurityContext::new(UserRole::Customer, Some(user_id));
        assert!(matches!(rate_limited_author(&security, 10), Ok(Some(id)) if id == user_id));
    }

    #[test]
    fn user_actor_without_an_id_is_rejected_not_exempted() {
        let security = SecurityContext::new(UserRole::Customer, None);
        assert!(rate_limited_author(&security, 10).is_err());
    }

    #[test]
    fn zero_interval_disables_the_check_for_every_actor() {
        let security = SecurityContext::new(UserRole::Customer, None);
        assert!(matches!(rate_limited_author(&security, 0), Ok(None)));
    }
}
