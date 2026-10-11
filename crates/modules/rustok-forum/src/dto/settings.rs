use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumModuleSettings {
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
    pub allow_user_topic_closing: bool,
    #[serde(default)]
    pub allow_user_content_deletion: bool,

    // Pagination & Display
    #[serde(default = "default_per_page")]
    pub topics_per_page: u32,
    #[serde(default = "default_per_page")]
    pub replies_per_page: u32,
    #[serde(default = "default_topic_sort")]
    pub default_topic_sort: String,
    #[serde(default = "default_true")]
    pub show_locked_topics_in_lists: bool,

    // Voting, Reactions & Engagement
    #[serde(default, alias = "useReactions")]
    pub use_reactions: bool,
    #[serde(default = "default_true")]
    pub allow_downvotes: bool,
    #[serde(default)]
    pub allow_self_voting: bool,

    // Moderation & Access
    #[serde(default)]
    pub pre_moderation_enabled: bool,
    #[serde(default = "default_true")]
    pub allow_anonymous_reading: bool,
    #[serde(default)]
    pub max_edit_window_minutes: u32,
}

impl Default for ForumModuleSettings {
    fn default() -> Self {
        Self {
            min_topic_title_length: default_min_topic_title_length(),
            max_topic_title_length: default_max_topic_title_length(),
            min_post_body_length: default_min_post_body_length(),
            max_post_body_length: default_max_post_body_length(),
            rate_limit_new_topic_seconds: default_rate_limit_new_topic_seconds(),
            rate_limit_new_reply_seconds: default_rate_limit_new_reply_seconds(),
            allow_user_topic_closing: false,
            allow_user_content_deletion: false,
            topics_per_page: default_per_page(),
            replies_per_page: default_per_page(),
            default_topic_sort: default_topic_sort(),
            show_locked_topics_in_lists: true,
            use_reactions: false,
            allow_downvotes: true,
            allow_self_voting: false,
            pre_moderation_enabled: false,
            allow_anonymous_reading: true,
            max_edit_window_minutes: 0,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_min_topic_title_length() -> u32 {
    1
}

fn default_max_topic_title_length() -> u32 {
    255
}

fn default_min_post_body_length() -> u32 {
    1
}

fn default_max_post_body_length() -> u32 {
    60000
}

fn default_rate_limit_new_topic_seconds() -> u32 {
    10
}

fn default_rate_limit_new_reply_seconds() -> u32 {
    5
}

fn default_per_page() -> u32 {
    20
}

fn default_topic_sort() -> String {
    "latest_reply".to_string()
}
