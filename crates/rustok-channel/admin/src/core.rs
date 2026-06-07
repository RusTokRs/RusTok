//! Framework-agnostic helpers for the channel admin FFA boundary.
//!
//! This module owns small state/query policies that should stay reusable by
//! future host adapters instead of being embedded in the Leptos render layer.

use crate::model::ChannelAdminBootstrap;

/// Returns whether a URL-selected channel id is still present in the current
/// admin bootstrap payload.
pub(crate) fn channel_selection_exists(
    bootstrap: &ChannelAdminBootstrap,
    channel_id: &str,
) -> bool {
    bootstrap
        .channels
        .iter()
        .any(|channel| channel.channel.id == channel_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ChannelAdminBootstrap, ChannelDetail, ChannelRecord};

    fn bootstrap_with_channel(id: &str) -> ChannelAdminBootstrap {
        ChannelAdminBootstrap {
            channels: vec![ChannelDetail {
                channel: ChannelRecord {
                    id: id.to_string(),
                    tenant_id: "tenant".to_string(),
                    slug: "default".to_string(),
                    name: "Default".to_string(),
                    is_active: true,
                    is_default: true,
                    status: "active".to_string(),
                    settings: serde_json::json!({}),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                    updated_at: "2026-01-01T00:00:00Z".to_string(),
                },
                targets: vec![],
                module_bindings: vec![],
                oauth_apps: vec![],
            }],
            current_channel: None,
            policy_sets: vec![],
            available_modules: vec![],
            oauth_apps: vec![],
        }
    }

    #[test]
    fn detects_existing_selection() {
        let bootstrap = bootstrap_with_channel("channel-a");
        assert!(channel_selection_exists(&bootstrap, "channel-a"));
        assert!(!channel_selection_exists(&bootstrap, "channel-b"));
    }
}
