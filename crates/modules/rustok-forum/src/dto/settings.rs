use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumModuleSettings {
    // Submodules
    #[serde(default = "default_true")]
    pub submodule_subscriptions_enabled: bool,
    #[serde(default = "default_true")]
    pub submodule_moderation_enabled: bool,
    #[serde(default = "default_true")]
    pub submodule_attachments_enabled: bool,
    #[serde(default = "default_true")]
    pub submodule_mentions_enabled: bool,
    #[serde(default = "default_true")]
    pub submodule_search_enabled: bool,

    // Posting Limits
    #[serde(default = "default_min_topic_title_length")]
    pub min_topic_title_length: u32,
    #[serde(default = "default_max_topic_title_length")]
    pub max_topic_title_length: u32,
    #[serde(default = "default_min_post_body_length")]
    pub min_post_body_length: u32,
    #[serde(default = "default_max_post_body_length")]
    pub max_post_body_length: u32,
    #[serde(default = "default_rate_limit_new_topic_seconds")]
    pub rate_limit_new_topic_seconds: u32,
    #[serde(default = "default_rate_limit_new_reply_seconds")]
    pub rate_limit_new_reply_seconds: u32,
    #[serde(default)]
    pub auto_close_inactive_days: u32,
    #[serde(default)]
    pub allow_user_topic_closing: bool,

    // Pagination & Display
    #[serde(default = "default_per_page")]
    pub topics_per_page: u32,
    #[serde(default = "default_per_page")]
    pub replies_per_page: u32,
    #[serde(default = "default_topic_sort")]
    pub default_topic_sort: String,
    #[serde(default = "default_hot_topic_threshold_replies")]
    pub hot_topic_threshold_replies: u32,
    #[serde(default = "default_hot_topic_threshold_views")]
    pub hot_topic_threshold_views: u32,
    #[serde(default = "default_true")]
    pub show_locked_topics_in_lists: bool,

    // Voting, Reactions & Engagement
    #[serde(default, alias = "useReactions")]
    pub use_reactions: bool,
    #[serde(default = "default_true")]
    pub allow_downvotes: bool,
    #[serde(default)]
    pub allow_self_voting: bool,
    #[serde(default = "default_vote_undo_window_minutes")]
    pub vote_undo_window_minutes: u32,

    // Moderation & Access
    #[serde(default)]
    pub pre_moderation_enabled: bool,
    #[serde(default = "default_auto_flag_threshold")]
    pub auto_flag_threshold: u32,
    #[serde(default = "default_true")]
    pub allow_anonymous_reading: bool,
    #[serde(default = "default_edit_grace_period_minutes")]
    pub edit_grace_period_minutes: u32,
    #[serde(default)]
    pub max_edit_window_minutes: u32,
}

impl Default for ForumModuleSettings {
    fn default() -> Self {
        Self {
            submodule_subscriptions_enabled: true,
            submodule_moderation_enabled: true,
            submodule_attachments_enabled: true,
            submodule_mentions_enabled: true,
            submodule_search_enabled: true,
            min_topic_title_length: default_min_topic_title_length(),
            max_topic_title_length: default_max_topic_title_length(),
            min_post_body_length: default_min_post_body_length(),
            max_post_body_length: default_max_post_body_length(),
            rate_limit_new_topic_seconds: default_rate_limit_new_topic_seconds(),
            rate_limit_new_reply_seconds: default_rate_limit_new_reply_seconds(),
            auto_close_inactive_days: 0,
            allow_user_topic_closing: false,
            topics_per_page: default_per_page(),
            replies_per_page: default_per_page(),
            default_topic_sort: default_topic_sort(),
            hot_topic_threshold_replies: default_hot_topic_threshold_replies(),
            hot_topic_threshold_views: default_hot_topic_threshold_views(),
            show_locked_topics_in_lists: true,
            use_reactions: false,
            allow_downvotes: true,
            allow_self_voting: false,
            vote_undo_window_minutes: default_vote_undo_window_minutes(),
            pre_moderation_enabled: false,
            auto_flag_threshold: default_auto_flag_threshold(),
            allow_anonymous_reading: true,
            edit_grace_period_minutes: default_edit_grace_period_minutes(),
            max_edit_window_minutes: 0,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_min_topic_title_length() -> u32 {
    5
}

fn default_max_topic_title_length() -> u32 {
    150
}

fn default_min_post_body_length() -> u32 {
    10
}

fn default_max_post_body_length() -> u32 {
    30000
}

fn default_rate_limit_new_topic_seconds() -> u32 {
    60
}

fn default_rate_limit_new_reply_seconds() -> u32 {
    15
}

fn default_per_page() -> u32 {
    20
}

fn default_topic_sort() -> String {
    "latest_reply".to_string()
}

fn default_hot_topic_threshold_replies() -> u32 {
    15
}

fn default_hot_topic_threshold_views() -> u32 {
    100
}

fn default_vote_undo_window_minutes() -> u32 {
    10
}

fn default_auto_flag_threshold() -> u32 {
    3
}

fn default_edit_grace_period_minutes() -> u32 {
    5
}
