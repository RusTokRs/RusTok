//! Shared derived-state effects of a reply crossing the public-status boundary.
//! Callers hold the category/topic/solution locks and persist the new status first.

use sea_orm::{ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::{forum_reply, forum_solution};
use crate::error::ForumResult;

use super::{UserStatsService, category::CategoryService, topic_facade::TopicService};

pub(crate) async fn apply_publication_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    reply: &forum_reply::Model,
    became_public: bool,
) -> ForumResult<Uuid> {
    let solution_delta = if !became_public {
        let removed = forum_solution::Entity::delete_many()
            .filter(forum_solution::Column::TenantId.eq(tenant_id))
            .filter(forum_solution::Column::TopicId.eq(reply.topic_id))
            .filter(forum_solution::Column::ReplyId.eq(reply.id))
            .exec(txn)
            .await?;
        if removed.rows_affected > 0 { -1 } else { 0 }
    } else {
        0
    };
    let delta = if became_public { 1 } else { -1 };
    let topic =
        TopicService::adjust_reply_count_in_tx(txn, tenant_id, reply.topic_id, delta).await?;
    // Archived topics retain their own approved-reply count, but have already
    // released their category/member contribution. Deleting a reply there must
    // not release those aggregates a second time.
    if topic.status == crate::state_machine::TopicStatus::Archived {
        return Ok(topic.category_id);
    }
    CategoryService::adjust_counters_in_tx(txn, tenant_id, topic.category_id, 0, delta).await?;
    // One statistics write: its guard recalculates every counter. Separate
    // solution/reply decrements would recalculate reply_count twice and reject
    // the second decrement when the author's last public reply was the solution.
    UserStatsService::adjust_reply_publication_counts_in_tx(
        txn,
        tenant_id,
        reply.author_id,
        delta,
        solution_delta,
    )
    .await?;
    Ok(topic.category_id)
}
