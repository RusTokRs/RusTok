use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::subscription::{ForumDigestMode, ForumSubscriptionLevel};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ForumSubscriptionTargetType {
    Category,
    Topic,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateForumSubscriptionInput {
    pub level: ForumSubscriptionLevel,
    pub notify_mentions: Option<bool>,
    pub notify_replies: Option<bool>,
    pub notify_new_topics: Option<bool>,
    pub digest_mode: Option<ForumDigestMode>,
    pub expected_revision: Option<i64>,
}

impl UpdateForumSubscriptionInput {
    pub fn watching() -> Self {
        Self {
            level: ForumSubscriptionLevel::Watching,
            notify_mentions: None,
            notify_replies: None,
            notify_new_topics: None,
            digest_mode: None,
            expected_revision: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumSubscriptionResponse {
    pub tenant_id: Uuid,
    pub target_type: ForumSubscriptionTargetType,
    pub target_id: Uuid,
    pub user_id: Uuid,
    pub level: ForumSubscriptionLevel,
    pub notify_mentions: bool,
    pub notify_replies: bool,
    pub notify_new_topics: bool,
    pub digest_mode: ForumDigestMode,
    pub last_notified_at: Option<String>,
    pub revision: i64,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateForumSubscriptionPolicyInput {
    pub auto_subscribe_topic_authors: bool,
    pub topic_author_level: ForumSubscriptionLevel,
    pub auto_subscribe_reply_participants: bool,
    pub reply_participant_level: ForumSubscriptionLevel,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForumSubscriptionPolicyResponse {
    pub tenant_id: Uuid,
    pub auto_subscribe_topic_authors: bool,
    pub topic_author_level: ForumSubscriptionLevel,
    pub auto_subscribe_reply_participants: bool,
    pub reply_participant_level: ForumSubscriptionLevel,
    pub revision: i64,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
