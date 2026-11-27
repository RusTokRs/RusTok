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
    // Opt-in window-count rules for the posting policy evaluator. Absent (`None`,
    // the default) keeps the evaluator disengaged and the flat cooldown above as
    // the only enforced limit. See
    // `DECISIONS/2026-11-27-forum-posting-policy-window-enforcement.md`.
    #[serde(default)]
    pub posting_policy_topic_create_limit: Option<ForumPostingWindowLimitSettings>,
    #[serde(default)]
    pub posting_policy_reply_create_limit: Option<ForumPostingWindowLimitSettings>,
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

/// Tenant-configured create-window rule consumed by the posting policy
/// evaluator (`topic_create_limit` / `reply_create_limit`). Bounded by
/// [`ForumModuleSettings::validate`] at settings write time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForumPostingWindowLimitSettings {
    /// Maximum creates allowed inside the window. Must be greater than zero.
    pub maximum_count: u32,
    /// Observation window length in seconds. Must be greater than zero and at
    /// most 30 days.
    pub window_seconds: u32,
}

/// Strongly typed error returned when validating forum module settings fails.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("{0}")]
pub struct ForumSettingsValidationError(pub String);

impl From<&str> for ForumSettingsValidationError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ForumSettingsValidationError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::ops::Deref for ForumSettingsValidationError {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ForumModuleSettings {
    /// Tenant-settings invariant: every combination that would make every
    /// create/update fail closed must be rejected at write time, not at
    /// runtime. This validator is the single owner for cross-field forum
    /// settings; the host `put_settings` path should call it before
    /// persisting.
    pub fn validate(&self) -> Result<(), ForumSettingsValidationError> {
        if self.max_topic_title_length != 0
            && self.min_topic_title_length != 0
            && self.min_topic_title_length > self.max_topic_title_length
        {
            return Err(format!(
                "min_topic_title_length ({}) cannot exceed max_topic_title_length ({})",
                self.min_topic_title_length, self.max_topic_title_length
            )
            .into());
        }
        if self.max_post_body_length != 0
            && self.min_post_body_length != 0
            && self.min_post_body_length > self.max_post_body_length
        {
            return Err(format!(
                "min_post_body_length ({}) cannot exceed max_post_body_length ({})",
                self.min_post_body_length, self.max_post_body_length
            )
            .into());
        }
        if self.topics_per_page == 0 || self.topics_per_page > 100 {
            return Err(format!(
                "topics_per_page must be between 1 and 100, got {}",
                self.topics_per_page
            )
            .into());
        }
        if self.replies_per_page == 0 || self.replies_per_page > 100 {
            return Err(format!(
                "replies_per_page must be between 1 and 100, got {}",
                self.replies_per_page
            )
            .into());
        }
        if self.default_topic_sort != "latest_reply" && self.default_topic_sort != "top" {
            return Err(format!(
                "default_topic_sort must be 'latest_reply' or 'top', got '{}'",
                self.default_topic_sort
            )
            .into());
        }
        if self.rate_limit_new_topic_seconds > 86_400 {
            return Err(format!(
                "rate_limit_new_topic_seconds must be <= 86400, got {}",
                self.rate_limit_new_topic_seconds
            )
            .into());
        }
        if self.rate_limit_new_reply_seconds > 86_400 {
            return Err(format!(
                "rate_limit_new_reply_seconds must be <= 86400, got {}",
                self.rate_limit_new_reply_seconds
            )
            .into());
        }
        if self.max_edit_window_minutes > 525_600 {
            return Err(format!(
                "max_edit_window_minutes must be <= 525600 (1 year), got {}",
                self.max_edit_window_minutes
            )
            .into());
        }
        for (label, limit) in [
            (
                "posting_policy_topic_create_limit",
                self.posting_policy_topic_create_limit,
            ),
            (
                "posting_policy_reply_create_limit",
                self.posting_policy_reply_create_limit,
            ),
        ] {
            if let Some(limit) = limit {
                if limit.maximum_count == 0 || limit.maximum_count > 1_000_000 {
                    return Err(format!(
                        "{label}.maximum_count must be between 1 and 1000000, got {}",
                        limit.maximum_count
                    )
                    .into());
                }
                if limit.window_seconds == 0 || limit.window_seconds > 2_592_000 {
                    return Err(format!(
                        "{label}.window_seconds must be between 1 and 2592000 (30 days), got {}",
                        limit.window_seconds
                    )
                    .into());
                }
            }
        }
        Ok(())
    }
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
            posting_policy_topic_create_limit: None,
            posting_policy_reply_create_limit: None,
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

#[cfg(test)]
mod posting_policy_settings_tests {
    use super::*;

    #[test]
    fn old_settings_json_has_no_window_rules() {
        let mut json = serde_json::to_value(ForumModuleSettings::default()).expect("settings JSON");
        let object = json.as_object_mut().expect("settings object");
        object.remove("posting_policy_topic_create_limit");
        object.remove("posting_policy_reply_create_limit");
        let settings: ForumModuleSettings = serde_json::from_value(json).expect("old settings");
        assert!(settings.posting_policy_topic_create_limit.is_none());
        assert!(settings.posting_policy_reply_create_limit.is_none());
        settings.validate().expect("default settings remain valid");
    }

    #[test]
    fn window_settings_accept_inclusive_bounds_and_round_trip() {
        for (maximum_count, window_seconds) in [(1, 1), (1_000_000, 2_592_000)] {
            let limit = Some(ForumPostingWindowLimitSettings {
                maximum_count,
                window_seconds,
            });
            let settings = ForumModuleSettings {
                posting_policy_topic_create_limit: limit,
                posting_policy_reply_create_limit: limit,
                ..ForumModuleSettings::default()
            };
            settings.validate().expect("inclusive bounds");
            let json = serde_json::to_value(&settings).expect("serialize");
            let decoded: ForumModuleSettings = serde_json::from_value(json).expect("deserialize");
            assert_eq!(decoded, settings);
        }
    }

    #[test]
    fn each_window_setting_rejects_zero_and_excessive_values() {
        for (maximum_count, window_seconds) in [
            (0, 1),
            (1_000_001, 1),
            (1, 0),
            (1, 2_592_001),
        ] {
            let limit = Some(ForumPostingWindowLimitSettings {
                maximum_count,
                window_seconds,
            });
            for topic in [true, false] {
                let mut settings = ForumModuleSettings::default();
                let label = if topic {
                    settings.posting_policy_topic_create_limit = limit;
                    "posting_policy_topic_create_limit"
                } else {
                    settings.posting_policy_reply_create_limit = limit;
                    "posting_policy_reply_create_limit"
                };
                let error = settings.validate().expect_err("invalid window setting");
                assert!(error.to_string().contains(label));
            }
        }
    }
}
