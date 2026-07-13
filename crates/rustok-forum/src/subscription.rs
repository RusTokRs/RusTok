use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ForumSubscriptionLevel {
    #[sea_orm(string_value = "watching")]
    Watching,
    #[sea_orm(string_value = "tracking")]
    Tracking,
    #[sea_orm(string_value = "normal")]
    Normal,
    #[sea_orm(string_value = "muted")]
    Muted,
}

impl ForumSubscriptionLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::Tracking => "tracking",
            Self::Normal => "normal",
            Self::Muted => "muted",
        }
    }

    pub const fn is_explicitly_subscribed(self) -> bool {
        matches!(self, Self::Watching | Self::Tracking)
    }

    pub const fn default_preferences(self) -> ForumSubscriptionPreferences {
        match self {
            Self::Watching => ForumSubscriptionPreferences {
                notify_mentions: true,
                notify_replies: true,
                notify_new_topics: true,
                digest_mode: ForumDigestMode::Immediate,
            },
            Self::Tracking => ForumSubscriptionPreferences {
                notify_mentions: true,
                notify_replies: false,
                notify_new_topics: false,
                digest_mode: ForumDigestMode::Disabled,
            },
            Self::Normal => ForumSubscriptionPreferences {
                notify_mentions: true,
                notify_replies: false,
                notify_new_topics: false,
                digest_mode: ForumDigestMode::Disabled,
            },
            Self::Muted => ForumSubscriptionPreferences {
                notify_mentions: false,
                notify_replies: false,
                notify_new_topics: false,
                digest_mode: ForumDigestMode::Disabled,
            },
        }
    }
}

impl std::fmt::Display for ForumSubscriptionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ForumDigestMode {
    #[sea_orm(string_value = "immediate")]
    Immediate,
    #[sea_orm(string_value = "daily")]
    Daily,
    #[sea_orm(string_value = "weekly")]
    Weekly,
    #[sea_orm(string_value = "disabled")]
    Disabled,
}

impl ForumDigestMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Disabled => "disabled",
        }
    }
}

impl std::fmt::Display for ForumDigestMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForumSubscriptionPreferences {
    pub notify_mentions: bool,
    pub notify_replies: bool,
    pub notify_new_topics: bool,
    pub digest_mode: ForumDigestMode,
}

impl ForumSubscriptionPreferences {
    pub fn resolve(
        level: ForumSubscriptionLevel,
        notify_mentions: Option<bool>,
        notify_replies: Option<bool>,
        notify_new_topics: Option<bool>,
        digest_mode: Option<ForumDigestMode>,
    ) -> Self {
        if level == ForumSubscriptionLevel::Muted {
            return level.default_preferences();
        }
        let defaults = level.default_preferences();
        Self {
            notify_mentions: notify_mentions.unwrap_or(defaults.notify_mentions),
            notify_replies: notify_replies.unwrap_or(defaults.notify_replies),
            notify_new_topics: notify_new_topics.unwrap_or(defaults.notify_new_topics),
            digest_mode: digest_mode.unwrap_or(defaults.digest_mode),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_always_disables_delivery_preferences() {
        let preferences = ForumSubscriptionPreferences::resolve(
            ForumSubscriptionLevel::Muted,
            Some(true),
            Some(true),
            Some(true),
            Some(ForumDigestMode::Daily),
        );
        assert!(!preferences.notify_mentions);
        assert!(!preferences.notify_replies);
        assert!(!preferences.notify_new_topics);
        assert_eq!(preferences.digest_mode, ForumDigestMode::Disabled);
    }

    #[test]
    fn compatibility_subscription_flag_only_tracks_watching_and_tracking() {
        assert!(ForumSubscriptionLevel::Watching.is_explicitly_subscribed());
        assert!(ForumSubscriptionLevel::Tracking.is_explicitly_subscribed());
        assert!(!ForumSubscriptionLevel::Normal.is_explicitly_subscribed());
        assert!(!ForumSubscriptionLevel::Muted.is_explicitly_subscribed());
    }
}
