use crate::i18n;
use crate::model::{
    ChannelListItemPayload, CreateChannelPayload, E2eeDeviceAuditPayload, MessengerOverviewPayload,
};

/// Navigation tabs in the Messenger Admin dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessengerAdminTab {
    #[default]
    Overview,
    Channels,
    E2ee,
    Federation,
}

impl MessengerAdminTab {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Channels => "channels",
            Self::E2ee => "e2ee",
            Self::Federation => "federation",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "channels" => Self::Channels,
            "e2ee" => Self::E2ee,
            "federation" => Self::Federation,
            _ => Self::Overview,
        }
    }
}

/// Draft input for creating a new channel/conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelFormDraft {
    pub conversation_type: String,
    pub title: String,
    pub topic: String,
    pub locale: String,
}

impl Default for ChannelFormDraft {
    fn default() -> Self {
        Self {
            conversation_type: "channel".to_string(),
            title: String::new(),
            topic: String::new(),
            locale: "en".to_string(),
        }
    }
}

impl ChannelFormDraft {
    /// Validates channel draft inputs.
    pub fn validate(&self) -> Result<CreateChannelPayload, String> {
        let title = self.title.trim();
        if title.is_empty() {
            return Err("Channel title cannot be empty.".to_string());
        }
        if title.len() < 2 {
            return Err("Channel title must be at least 2 characters.".to_string());
        }
        if title.len() > 100 {
            return Err("Channel title cannot exceed 100 characters.".to_string());
        }

        let topic = if self.topic.trim().is_empty() {
            None
        } else {
            Some(self.topic.trim().to_string())
        };

        let conv_type = match self.conversation_type.trim().to_lowercase().as_str() {
            "channel" => "channel",
            "group" => "group",
            "direct" => "direct",
            "topic" => "topic",
            other => return Err(format!("Unsupported conversation type: '{other}'.")),
        };

        let locale = if self.locale.trim().is_empty() {
            "en".to_string()
        } else {
            self.locale.trim().to_lowercase()
        };

        Ok(CreateChannelPayload {
            conversation_type: conv_type.to_string(),
            title: title.to_string(),
            topic,
            locale,
        })
    }
}

/// Computed view-model for channel list entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelListItemViewModel {
    pub id_display: String,
    pub raw_id: uuid::Uuid,
    pub type_label: &'static str,
    pub type_badge_variant: &'static str,
    pub title_display: String,
    pub topic_display: String,
    pub member_count: usize,
    pub created_at_display: String,
}

impl ChannelListItemViewModel {
    pub fn from_payload(p: &ChannelListItemPayload) -> Self {
        let (type_label, type_badge_variant) = match p.conversation_type.as_str() {
            "channel" => ("Channel", "primary"),
            "group" => ("Group", "default"),
            "direct" => ("Direct", "secondary"),
            "topic" => ("Topic", "outline"),
            _ => ("Custom", "outline"),
        };

        let title_display = p
            .title
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or("Untitled Conversation")
            .to_string();

        let topic_display = p
            .topic
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or("—")
            .to_string();

        Self {
            id_display: p.id.to_string()[..8].to_string(),
            raw_id: p.id,
            type_label,
            type_badge_variant,
            title_display,
            topic_display,
            member_count: p.member_count,
            created_at_display: p.created_at.format("%Y-%m-%d %H:%M").to_string(),
        }
    }
}

/// Computed view-model for E2EE device key audits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct E2eeDeviceAuditViewModel {
    pub device_id: String,
    pub user_id_display: String,
    pub has_keys_badge: &'static str,
    pub otk_count: usize,
    pub otk_status: &'static str,
    pub otk_badge_variant: &'static str,
}

impl E2eeDeviceAuditViewModel {
    pub fn from_payload(p: &E2eeDeviceAuditPayload) -> Self {
        let (otk_status, otk_badge_variant) = if p.otk_count > 20 {
            ("Adequate", "success")
        } else if p.otk_count > 0 {
            ("Low", "warning")
        } else {
            ("Depleted", "destructive")
        };

        let has_keys_badge = if p.has_curve25519 && p.has_ed25519 {
            "Valid (Curve25519+Ed25519)"
        } else {
            "Incomplete Keys"
        };

        Self {
            device_id: p.device_id.clone(),
            user_id_display: p.user_id.to_string()[..8].to_string(),
            has_keys_badge,
            otk_count: p.otk_count,
            otk_status,
            otk_badge_variant,
        }
    }
}

/// Localized labels resolved once at UI component mount.
#[derive(Debug, Clone)]
pub struct MessengerAdminLabels {
    pub eyebrow: String,
    pub title: String,
    pub subtitle: String,
    pub tab_overview: String,
    pub tab_channels: String,
    pub tab_e2ee: String,
    pub tab_federation: String,
    pub stat_conversations: String,
    pub stat_streaming: String,
    pub stat_e2ee_devices: String,
    pub stat_matrix: String,
    pub federation_status: String,
    pub streaming_title: String,
    pub streaming_topic: String,
    pub streaming_group: String,
    pub streaming_partitions: String,
    pub channels_title: String,
    pub channels_id: String,
    pub channels_type: String,
    pub channels_title_col: String,
    pub channels_topic_col: String,
    pub channels_created: String,
    pub channels_empty: String,
    pub e2ee_badge: String,
    pub e2ee_description: String,
    pub e2ee_title: String,
    pub e2ee_device_id: String,
    pub e2ee_user_id: String,
    pub e2ee_otk_pool: String,
    pub e2ee_empty: String,
    pub federation_title: String,
    pub federation_server: String,
    pub federation_rooms: String,
}

impl MessengerAdminLabels {
    pub fn resolve(locale: Option<&str>) -> Self {
        Self {
            eyebrow: i18n::t(locale, "messenger-eyebrow", "Communication"),
            title: i18n::t(locale, "messenger-title", "Messenger & Real-Time Chat"),
            subtitle: i18n::t(
                locale,
                "messenger-subtitle",
                "Channels, direct messaging, streaming fabric, and E2EE key registry",
            ),
            tab_overview: i18n::t(locale, "messenger-tab-overview", "Overview & Fabric"),
            tab_channels: i18n::t(locale, "messenger-tab-channels", "Channels & Conversations"),
            tab_e2ee: i18n::t(locale, "messenger-tab-e2ee", "E2EE & Cryptography"),
            tab_federation: i18n::t(locale, "messenger-tab-federation", "Matrix Federation"),
            stat_conversations: i18n::t(
                locale,
                "messenger-stat-conversations",
                "Active Conversations",
            ),
            stat_streaming: i18n::t(locale, "messenger-stat-streaming", "Streaming Partitions"),
            stat_e2ee_devices: i18n::t(
                locale,
                "messenger-stat-e2ee-devices",
                "Registered E2EE Devices",
            ),
            stat_matrix: i18n::t(locale, "messenger-stat-matrix", "Federation Bridge"),
            federation_status: i18n::t(
                locale,
                "messenger-federation-status",
                "Gateway Operational",
            ),
            streaming_title: i18n::t(locale, "messenger-streaming-title", "Iggy Streaming Fabric"),
            streaming_topic: i18n::t(locale, "messenger-streaming-topic", "Stream Topic"),
            streaming_group: i18n::t(locale, "messenger-streaming-group", "Consumer Group"),
            streaming_partitions: i18n::t(locale, "messenger-streaming-partitions", "Partitions"),
            channels_title: i18n::t(locale, "messenger-channels-title", "Channels Management"),
            channels_id: i18n::t(locale, "messenger-channels-id", "ID"),
            channels_type: i18n::t(locale, "messenger-channels-type", "Type"),
            channels_title_col: i18n::t(locale, "messenger-channels-title-col", "Title"),
            channels_topic_col: i18n::t(locale, "messenger-channels-topic-col", "Topic"),
            channels_created: i18n::t(locale, "messenger-channels-created", "Created At"),
            channels_empty: i18n::t(
                locale,
                "messenger-channels-empty",
                "No channels or conversations found.",
            ),
            e2ee_badge: i18n::t(
                locale,
                "messenger-e2ee-badge",
                "Zero-Knowledge Server Blindness Guaranteed",
            ),
            e2ee_description: i18n::t(
                locale,
                "messenger-e2ee-description",
                "The server acts strictly as an untrusted blind relay. Master keys, identity private keys, and double ratchet states exist exclusively in client secure enclaves.",
            ),
            e2ee_title: i18n::t(
                locale,
                "messenger-e2ee-title",
                "Zero-Knowledge Cryptographic Key Directory",
            ),
            e2ee_device_id: i18n::t(locale, "messenger-e2ee-device-id", "Device ID"),
            e2ee_user_id: i18n::t(locale, "messenger-e2ee-user-id", "User ID"),
            e2ee_otk_pool: i18n::t(locale, "messenger-e2ee-otk-pool", "OTK Pool"),
            e2ee_empty: "No client devices registered yet.".to_string(),
            federation_title: i18n::t(
                locale,
                "messenger-federation-title",
                "Matrix Federation Gateway",
            ),
            federation_server: i18n::t(locale, "messenger-federation-server", "Server Domain"),
            federation_rooms: i18n::t(locale, "messenger-federation-rooms", "Federated Rooms"),
        }
    }
}

/// Fallback overview payload for offline or mock state.
pub fn default_overview_payload() -> MessengerOverviewPayload {
    MessengerOverviewPayload {
        active_conversations_count: 0,
        stream_topic: "rustok-messenger-events".to_string(),
        consumer_group: "rustok-messenger-workers".to_string(),
        partitions_count: 16,
        registered_devices_count: 0,
        matrix_server_name: "rustok.org".to_string(),
        federated_rooms_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn admin_tab_from_and_as_str() {
        assert_eq!(
            MessengerAdminTab::from_str("overview"),
            MessengerAdminTab::Overview
        );
        assert_eq!(
            MessengerAdminTab::from_str("channels"),
            MessengerAdminTab::Channels
        );
        assert_eq!(MessengerAdminTab::from_str("e2ee"), MessengerAdminTab::E2ee);
        assert_eq!(
            MessengerAdminTab::from_str("federation"),
            MessengerAdminTab::Federation
        );
        assert_eq!(
            MessengerAdminTab::from_str("unknown"),
            MessengerAdminTab::Overview
        );

        assert_eq!(MessengerAdminTab::Overview.as_str(), "overview");
        assert_eq!(MessengerAdminTab::Channels.as_str(), "channels");
    }

    #[test]
    fn channel_draft_validation_success() {
        let draft = ChannelFormDraft {
            conversation_type: "channel".into(),
            title: "Engineering Team".into(),
            topic: "Architecture discussions".into(),
            locale: "en".into(),
        };

        let res = draft.validate().expect("draft must be valid");
        assert_eq!(res.title, "Engineering Team");
        assert_eq!(res.topic, Some("Architecture discussions".into()));
        assert_eq!(res.conversation_type, "channel");
        assert_eq!(res.locale, "en");
    }

    #[test]
    fn channel_draft_validation_rejections() {
        let empty_title = ChannelFormDraft {
            title: "  ".into(),
            ..Default::default()
        };
        assert!(empty_title.validate().is_err());

        let short_title = ChannelFormDraft {
            title: "A".into(),
            ..Default::default()
        };
        assert!(short_title.validate().is_err());

        let invalid_type = ChannelFormDraft {
            title: "Valid Title".into(),
            conversation_type: "invalid_type".into(),
            ..Default::default()
        };
        assert!(invalid_type.validate().is_err());
    }

    #[test]
    fn view_model_channel_item_projection() {
        let payload = ChannelListItemPayload {
            id: Uuid::now_v7(),
            conversation_type: "channel".into(),
            title: Some("General Chat".into()),
            topic: None,
            member_count: 42,
            created_at: Utc::now(),
        };

        let vm = ChannelListItemViewModel::from_payload(&payload);
        assert_eq!(vm.type_label, "Channel");
        assert_eq!(vm.title_display, "General Chat");
        assert_eq!(vm.topic_display, "—");
        assert_eq!(vm.member_count, 42);
    }

    #[test]
    fn view_model_e2ee_device_audit() {
        let payload = E2eeDeviceAuditPayload {
            device_id: "DEVICE_01".into(),
            user_id: Uuid::now_v7(),
            has_curve25519: true,
            has_ed25519: true,
            otk_count: 50,
        };

        let vm = E2eeDeviceAuditViewModel::from_payload(&payload);
        assert_eq!(vm.otk_status, "Adequate");
        assert_eq!(vm.otk_badge_variant, "success");
        assert_eq!(vm.has_keys_badge, "Valid (Curve25519+Ed25519)");
    }

    #[test]
    fn labels_resolution_fallback() {
        let labels = MessengerAdminLabels::resolve(Some("en"));
        assert_eq!(labels.title, "Messenger & Real-Time Chat");
        assert_eq!(labels.eyebrow, "Communication");
    }
}
