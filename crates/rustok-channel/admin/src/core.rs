//! Framework-agnostic helpers for the channel admin FFA boundary.
//!
//! This module owns small state/query policies that should stay reusable by
//! future host adapters instead of being embedded in a framework render layer.

use crate::model::{
    ChannelAdminBootstrap, ChannelDetail, ChannelResolutionPolicySetDetail,
    ChannelResolutionPredicateRecord, ChannelResolutionRuleRecord, CreateResolutionRulePayload,
    UpdateResolutionRulePayload,
};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChannelPolicySelectionCleanup {
    None,
    ClearRule,
    ClearPolicySetAndRule,
}

pub(crate) fn channel_policy_selection_cleanup(
    policy_sets: &[ChannelResolutionPolicySetDetail],
    selected_policy_set_id: Option<&str>,
    selected_policy_rule_id: Option<&str>,
) -> ChannelPolicySelectionCleanup {
    let selected_policy_set_id = selected_policy_set_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let selected_policy_rule_id = selected_policy_rule_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(policy_set_id) = selected_policy_set_id else {
        return if selected_policy_rule_id.is_some() {
            ChannelPolicySelectionCleanup::ClearRule
        } else {
            ChannelPolicySelectionCleanup::None
        };
    };

    let Some(policy_set) = policy_sets
        .iter()
        .find(|policy_set| policy_set.policy_set.id == policy_set_id)
    else {
        return ChannelPolicySelectionCleanup::ClearPolicySetAndRule;
    };

    match selected_policy_rule_id {
        Some(rule_id) if !policy_set.rules.iter().any(|rule| rule.id == rule_id) => {
            ChannelPolicySelectionCleanup::ClearRule
        }
        _ => ChannelPolicySelectionCleanup::None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyRuleFormState {
    pub priority: i32,
    pub is_active: bool,
    pub action_channel_id: String,
    pub host_equals: String,
    pub host_suffix: String,
    pub oauth_app_id: String,
    pub surface: String,
    pub locale: String,
}

impl PolicyRuleFormState {
    pub(crate) fn create_payload(&self) -> CreateResolutionRulePayload {
        CreateResolutionRulePayload {
            priority: self.priority,
            is_active: self.is_active,
            action_channel_id: self.action_channel_id.clone(),
            host_equals: normalized_optional_text(&self.host_equals),
            host_suffix: normalized_optional_text(&self.host_suffix),
            oauth_app_id: normalized_optional_text(&self.oauth_app_id),
            surface: normalized_optional_text(&self.surface),
            locale: normalized_optional_text(&self.locale),
        }
    }

    pub(crate) fn update_payload(&self) -> UpdateResolutionRulePayload {
        UpdateResolutionRulePayload {
            priority: Some(self.priority),
            is_active: Some(self.is_active),
            action_channel_id: Some(self.action_channel_id.clone()),
            host_equals: Some(self.host_equals.clone()),
            host_suffix: Some(self.host_suffix.clone()),
            oauth_app_id: Some(self.oauth_app_id.clone()),
            surface: Some(self.surface.clone()),
            locale: Some(self.locale.clone()),
        }
    }
}

fn normalized_optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn policy_rule_active_update_payload(is_active: bool) -> UpdateResolutionRulePayload {
    UpdateResolutionRulePayload {
        priority: None,
        is_active: Some(is_active),
        action_channel_id: None,
        host_equals: None,
        host_suffix: None,
        oauth_app_id: None,
        surface: None,
        locale: None,
    }
}

pub(crate) fn policy_rule_create_form_state(
    rules: &[ChannelResolutionRuleRecord],
    channels: &[ChannelDetail],
) -> PolicyRuleFormState {
    PolicyRuleFormState {
        priority: rules.last().map(|rule| rule.priority + 10).unwrap_or(10),
        is_active: true,
        action_channel_id: channels
            .first()
            .map(|channel| channel.channel.id.clone())
            .unwrap_or_default(),
        host_equals: String::new(),
        host_suffix: String::new(),
        oauth_app_id: String::new(),
        surface: "http".to_string(),
        locale: String::new(),
    }
}

pub(crate) fn policy_rule_edit_form_state(
    rule: &ChannelResolutionRuleRecord,
    channels: &[ChannelDetail],
) -> PolicyRuleFormState {
    let fallback_action_channel_id = channels
        .first()
        .map(|channel| channel.channel.id.clone())
        .unwrap_or_else(|| rule.action_channel_id.clone());

    let mut state = PolicyRuleFormState {
        priority: rule.priority,
        is_active: rule.is_active,
        action_channel_id: if channels
            .iter()
            .any(|channel| channel.channel.id == rule.action_channel_id)
        {
            rule.action_channel_id.clone()
        } else {
            fallback_action_channel_id
        },
        host_equals: String::new(),
        host_suffix: String::new(),
        oauth_app_id: String::new(),
        surface: String::new(),
        locale: String::new(),
    };

    for predicate in &rule.definition.predicates {
        match predicate {
            ChannelResolutionPredicateRecord::HostEquals(value) => {
                state.host_equals = value.clone()
            }
            ChannelResolutionPredicateRecord::HostSuffix(value) => {
                state.host_suffix = value.clone()
            }
            ChannelResolutionPredicateRecord::OAuthAppEquals(value) => {
                state.oauth_app_id = value.clone()
            }
            ChannelResolutionPredicateRecord::SurfaceIs(value) => state.surface = value.clone(),
            ChannelResolutionPredicateRecord::LocaleEquals(value) => state.locale = value.clone(),
        }
    }

    state
}

pub(crate) fn reorder_policy_rule_ids(
    rule_ids: &[String],
    index: usize,
    move_up: bool,
) -> Option<Vec<String>> {
    if move_up {
        if index == 0 || index >= rule_ids.len() {
            return None;
        }
    } else if index + 1 >= rule_ids.len() {
        return None;
    }

    let mut reordered = rule_ids.to_vec();
    if move_up {
        reordered.swap(index, index - 1);
    } else {
        reordered.swap(index, index + 1);
    }
    Some(reordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ChannelAdminBootstrap, ChannelDetail, ChannelRecord, ChannelResolutionPolicySetDetail,
        ChannelResolutionPolicySetRecord,
    };

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

    #[test]
    fn normalizes_policy_set_and_rule_selection_cleanup() {
        let policy_sets = vec![ChannelResolutionPolicySetDetail {
            policy_set: ChannelResolutionPolicySetRecord {
                id: "set-a".to_string(),
                tenant_id: "tenant".to_string(),
                slug: "default".to_string(),
                name: "Default".to_string(),
                schema_version: 1,
                is_active: true,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            rules: vec![],
        }];

        assert_eq!(
            channel_policy_selection_cleanup(&policy_sets, Some("missing"), Some("rule-a")),
            ChannelPolicySelectionCleanup::ClearPolicySetAndRule
        );
        assert_eq!(
            channel_policy_selection_cleanup(&policy_sets, Some(" set-a "), Some("missing")),
            ChannelPolicySelectionCleanup::ClearRule
        );
        assert_eq!(
            channel_policy_selection_cleanup(&policy_sets, None, Some("rule-a")),
            ChannelPolicySelectionCleanup::ClearRule
        );
        assert_eq!(
            channel_policy_selection_cleanup(&policy_sets, Some("set-a"), None),
            ChannelPolicySelectionCleanup::None
        );
    }

    #[test]
    fn reorders_policy_rule_ids_and_rejects_invalid_moves() {
        let base = vec![
            "rule-1".to_string(),
            "rule-2".to_string(),
            "rule-3".to_string(),
        ];

        assert_eq!(
            reorder_policy_rule_ids(&base, 1, true),
            Some(vec![
                "rule-2".to_string(),
                "rule-1".to_string(),
                "rule-3".to_string(),
            ])
        );
        assert_eq!(
            reorder_policy_rule_ids(&base, 1, false),
            Some(vec![
                "rule-1".to_string(),
                "rule-3".to_string(),
                "rule-2".to_string(),
            ])
        );
        assert_eq!(reorder_policy_rule_ids(&base, 0, true), None);
        assert_eq!(reorder_policy_rule_ids(&base, 2, false), None);
    }

    #[test]
    fn policy_rule_form_state_builds_create_update_and_active_payloads() {
        let state = PolicyRuleFormState {
            priority: 20,
            is_active: true,
            action_channel_id: "channel-a".to_string(),
            host_equals: " shop.example.test ".to_string(),
            host_suffix: " ".to_string(),
            oauth_app_id: String::new(),
            surface: " http ".to_string(),
            locale: " ru-by ".to_string(),
        };

        let create = state.create_payload();
        assert_eq!(create.host_equals.as_deref(), Some("shop.example.test"));
        assert_eq!(create.host_suffix, None);
        assert_eq!(create.surface.as_deref(), Some("http"));
        assert_eq!(create.locale.as_deref(), Some("ru-by"));

        let update = state.update_payload();
        assert_eq!(update.priority, Some(20));
        assert_eq!(update.host_equals.as_deref(), Some(" shop.example.test "));

        let active = policy_rule_active_update_payload(false);
        assert_eq!(active.is_active, Some(false));
        assert_eq!(active.priority, None);
    }
}
