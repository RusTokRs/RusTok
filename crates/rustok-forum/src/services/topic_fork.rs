mod category_audience {
    pub(super) use super::super::category_audience::lock_category_tree_in_tx;
}
mod projection_invalidation {
    pub(super) use super::super::projection_invalidation::{
        publish_forum_category_projection_in_tx, publish_forum_topic_projection_in_tx,
    };
}
mod rbac {
    pub(super) use super::super::rbac::enforce_scope;
}
mod topic {
    pub(super) use super::super::topic::MAX_FORUM_TOPIC_TAGS;
}
mod topic_audience {
    pub(super) use super::super::topic_audience::load_policy_for_topic;
}
mod topic_audience_lock {
    pub(super) use super::super::topic_audience_lock::lock_topic_audience_scopes_in_tx;
}
mod topic_reply_create_audience {
    pub(super) use super::super::topic_reply_create_audience::load_topic_reply_create_audience_policy_for_topic;
}
mod topic_solution_lock {
    use sea_orm::DatabaseTransaction;
    use uuid::Uuid;

    use crate::error::{ForumError, ForumResult};

    pub(super) async fn lock_topic_solution_scopes_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        topic_ids: &[Uuid],
    ) -> ForumResult<()> {
        let source_topic_id = topic_ids.first().copied().ok_or_else(|| {
            ForumError::Validation(
                "Forum topic fork solution lock requires a source topic".to_string(),
            )
        })?;
        super::super::topic_solution_lock::lock_topic_solution_scopes_in_tx(
            txn,
            tenant_id,
            &[source_topic_id],
        )
        .await
    }
}
mod topic_tag_lock {
    pub(super) use super::super::topic_tag_lock::lock_topic_tag_scopes_in_tx;
}
mod user_stats {
    pub(super) use super::super::user_stats::UserStatsService;
}

mod implementation {
    include!("topic_fork_owner.rs");
    include!("topic_fork_storage.rs");
}

pub use implementation::{
    ForkForumReplyBranchInput, ForumTopicForkResult, ForumTopicForkService,
    MAX_FORUM_TOPIC_FORK_BODY_ROWS, MAX_FORUM_TOPIC_FORK_MENTIONS, MAX_FORUM_TOPIC_FORK_QUOTES,
    MAX_FORUM_TOPIC_FORK_REASON_LEN, MAX_FORUM_TOPIC_FORK_RELATION_REVISIONS,
    MAX_FORUM_TOPIC_FORK_REPLIES, MAX_FORUM_TOPIC_FORK_REPLY_REVISIONS,
    MAX_FORUM_TOPIC_FORK_TITLE_LEN,
};
