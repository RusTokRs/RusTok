//! Framework-agnostic helpers for the channel admin FFA boundary.
//!
//! This module owns small state/query policies that should stay reusable by
//! future host adapters instead of being embedded in a framework render layer.

use crate::model::{
    ChannelAdminBootstrap, ChannelDetail, ChannelResolutionPolicySetDetail,
    ChannelResolutionPredicateRecord, ChannelResolutionRuleRecord, CreateResolutionRulePayload,
    UpdateChannelPayload, UpdateResolutionRulePayload,
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

/// Owner command and operator copy implied by a channel's activation state.
///
/// The lifecycle toggle is a state transition, not a view detail: which command to
/// send, which label to show and which feedback to report are derived here so the
/// Leptos adapter only binds values and never re-derives the transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChannelLifecycleToggle {
    pub(crate) next_is_active: bool,
    pub(crate) label_key: &'static str,
    pub(crate) label_fallback: &'static str,
    pub(crate) feedback_key: &'static str,
    pub(crate) feedback_fallback: &'static str,
}

pub(crate) fn channel_lifecycle_toggle(is_active: bool) -> ChannelLifecycleToggle {
    if is_active {
        ChannelLifecycleToggle {
            next_is_active: false,
            label_key: "channel.card.deactivate",
            label_fallback: "Deactivate",
            feedback_key: "channel.feedback.deactivated",
            feedback_fallback: "Channel `{slug}` is now inactive and no longer resolves.",
        }
    } else {
        ChannelLifecycleToggle {
            next_is_active: true,
            label_key: "channel.card.activate",
            label_fallback: "Activate",
            feedback_key: "channel.feedback.activated",
            feedback_fallback: "Channel `{slug}` is active again.",
        }
    }
}

/// Localized badge copy for a channel's publication state.
///
/// The admin never renders the raw wire value (`active`/`inactive`) and never keeps
/// its own copy of the state: `is_active` is the only fact, and the label is derived
/// from it in the same place that derives the lifecycle transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChannelStatusBadge {
    pub(crate) label_key: &'static str,
    pub(crate) label_fallback: &'static str,
}

pub(crate) fn channel_status_badge(is_active: bool) -> ChannelStatusBadge {
    if is_active {
        ChannelStatusBadge {
            label_key: "channel.card.statusActive",
            label_fallback: "Active",
        }
    } else {
        ChannelStatusBadge {
            label_key: "channel.card.statusInactive",
            label_fallback: "Inactive",
        }
    }
}

/// Operator copy reported after a lifecycle write that carries no extra payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChannelLifecycleFeedback {
    pub(crate) feedback_key: &'static str,
    pub(crate) feedback_fallback: &'static str,
}

/// Feedback for the confirmed delete action.
pub(crate) const CHANNEL_DELETE_FEEDBACK: ChannelLifecycleFeedback = ChannelLifecycleFeedback {
    feedback_key: "channel.feedback.deleted",
    feedback_fallback: "Channel `{slug}` was deleted.",
};

/// Owner command produced by the inline rename form.
///
/// Renaming is the only lifecycle write with user input, so its validation, the
/// `UpdateChannelPayload` it becomes and the feedback it reports are derived here.
/// The adapter holds the draft and calls the transport; it decides none of this.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChannelRenameCommand {
    pub(crate) payload: UpdateChannelPayload,
    pub(crate) feedback_key: &'static str,
    pub(crate) feedback_fallback: &'static str,
}

/// Message an adapter should show when a form value cannot become a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChannelFormMessage {
    pub(crate) key: &'static str,
    pub(crate) fallback: &'static str,
}

pub(crate) fn channel_rename_command(
    raw_name: &str,
) -> Result<ChannelRenameCommand, ChannelFormMessage> {
    let name = raw_name.trim();
    if name.is_empty() {
        return Err(ChannelFormMessage {
            key: "channel.error.nameRequired",
            fallback: "Channel name must not be empty",
        });
    }

    Ok(ChannelRenameCommand {
        payload: UpdateChannelPayload {
            name: Some(name.to_string()),
            settings: None,
        },
        feedback_key: "channel.feedback.renamed",
        feedback_fallback: "Channel `{slug}` was renamed.",
    })
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

/// Which workspace of the Channel admin the route subpath names.
///
/// The module declares `targets` and `apps` as child pages, and the host renders
/// one module component for all of them. Before this dispatch every subpath
/// rendered the identical stacked page, so both navigation entries were labels
/// without a destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelAdminSection {
    /// Landing workspace: every channel with all of its sections.
    Channels,
    /// Targets across all channels.
    Targets,
    /// Bound OAuth applications across all channels.
    Apps,
}

impl ChannelAdminSection {
    /// The `data-` value the root renders, so the active workspace is visible
    /// in the DOM and assertable without parsing markup.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Channels => "channels",
            Self::Targets => "targets",
            Self::Apps => "apps",
        }
    }

    pub(crate) fn shows_targets(self) -> bool {
        matches!(self, Self::Channels | Self::Targets)
    }

    pub(crate) fn shows_modules(self) -> bool {
        matches!(self, Self::Channels)
    }

    pub(crate) fn shows_oauth_apps(self) -> bool {
        matches!(self, Self::Channels | Self::Apps)
    }
}

/// Map a route subpath onto the workspace it names.
///
/// Unknown and nested subpaths fall back to the landing workspace: a stale
/// bookmark renders the module instead of an empty page.
pub fn channel_admin_section_for_subpath(subpath: Option<&str>) -> ChannelAdminSection {
    let Some(subpath) = subpath else {
        return ChannelAdminSection::Channels;
    };
    match subpath.trim_matches('/').split('/').next().unwrap_or("") {
        "targets" => ChannelAdminSection::Targets,
        "apps" => ChannelAdminSection::Apps,
        _ => ChannelAdminSection::Channels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ChannelAdminBootstrap, ChannelDetail, ChannelRecord, ChannelResolutionActionRecord,
        ChannelResolutionPolicySetDetail, ChannelResolutionPolicySetRecord,
        ChannelResolutionPredicateRecord, ChannelResolutionRuleDefinitionRecord,
        ChannelResolutionRuleRecord,
    };

    #[test]
    fn declared_child_pages_select_their_own_workspace() {
        assert_eq!(
            channel_admin_section_for_subpath(Some("targets")),
            ChannelAdminSection::Targets
        );
        assert_eq!(
            channel_admin_section_for_subpath(Some("apps")),
            ChannelAdminSection::Apps
        );
        assert_eq!(
            channel_admin_section_for_subpath(Some("targets/42")),
            ChannelAdminSection::Targets
        );
        assert_eq!(
            channel_admin_section_for_subpath(Some("channels")),
            ChannelAdminSection::Channels
        );
        assert_eq!(
            channel_admin_section_for_subpath(Some("unknown")),
            ChannelAdminSection::Channels
        );
        assert_eq!(
            channel_admin_section_for_subpath(None),
            ChannelAdminSection::Channels
        );
    }

    #[test]
    fn each_workspace_renders_exactly_its_own_card_sections() {
        // Landing renders everything; a focused workspace hides what it does not own.
        assert!(ChannelAdminSection::Channels.shows_targets());
        assert!(ChannelAdminSection::Channels.shows_modules());
        assert!(ChannelAdminSection::Channels.shows_oauth_apps());

        assert!(ChannelAdminSection::Targets.shows_targets());
        assert!(!ChannelAdminSection::Targets.shows_modules());
        assert!(!ChannelAdminSection::Targets.shows_oauth_apps());

        assert!(ChannelAdminSection::Apps.shows_oauth_apps());
        assert!(!ChannelAdminSection::Apps.shows_targets());
        assert!(!ChannelAdminSection::Apps.shows_modules());
    }

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

    #[test]
    fn channel_lifecycle_toggle_derives_the_owner_command_and_operator_copy() {
        let deactivate = channel_lifecycle_toggle(true);
        assert!(!deactivate.next_is_active);
        assert_eq!(deactivate.label_key, "channel.card.deactivate");
        assert_eq!(deactivate.feedback_key, "channel.feedback.deactivated");

        let activate = channel_lifecycle_toggle(false);
        assert!(activate.next_is_active);
        assert_eq!(activate.label_key, "channel.card.activate");
        assert_eq!(activate.feedback_key, "channel.feedback.activated");
    }

    #[test]
    fn channel_status_badge_is_derived_from_is_active() {
        let active = channel_status_badge(true);
        assert_eq!(active.label_key, "channel.card.statusActive");
        assert_eq!(active.label_fallback, "Active");

        let inactive = channel_status_badge(false);
        assert_eq!(inactive.label_key, "channel.card.statusInactive");
        assert_eq!(inactive.label_fallback, "Inactive");
    }

    #[test]
    fn channel_rename_command_trims_the_name_into_the_owner_payload() {
        let command = channel_rename_command("  Shop site  ").expect("a non-empty name is valid");
        assert_eq!(command.payload.name.as_deref(), Some("Shop site"));
        assert!(
            command.payload.settings.is_none(),
            "rename must not rewrite settings"
        );
        assert_eq!(command.feedback_key, "channel.feedback.renamed");
        assert_eq!(command.feedback_fallback, "Channel `{slug}` was renamed.");
    }

    #[test]
    fn channel_rename_command_rejects_a_blank_name_with_the_form_message() {
        for blank in ["", "   ", "\t\n"] {
            let error = channel_rename_command(blank).expect_err("a blank name must not be sent");
            assert_eq!(error.key, "channel.error.nameRequired");
            assert_eq!(error.fallback, "Channel name must not be empty");
        }
    }

    #[test]
    fn channel_delete_feedback_matches_the_catalog_key() {
        assert_eq!(CHANNEL_DELETE_FEEDBACK.feedback_key, "channel.feedback.deleted");
        assert_eq!(
            CHANNEL_DELETE_FEEDBACK.feedback_fallback,
            "Channel `{slug}` was deleted."
        );
    }

    #[test]
    fn policy_rule_create_form_state_initializes_with_next_priority_and_default_channel() {
        let channels = vec![ChannelDetail {
            channel: ChannelRecord {
                id: "channel_01".to_string(),
                tenant_id: "tenant_01".to_string(),
                slug: "web".to_string(),
                name: "Web".to_string(),
                is_active: true,
                is_default: true,
                settings: serde_json::json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            targets: Vec::new(),
            module_bindings: Vec::new(),
            oauth_apps: Vec::new(),
        }];

        let empty_rules = Vec::new();
        let state_empty = policy_rule_create_form_state(&empty_rules, &channels);
        assert_eq!(state_empty.priority, 10);
        assert!(state_empty.is_active);
        assert_eq!(state_empty.action_channel_id, "channel_01");
        assert_eq!(state_empty.surface, "http");

        let existing_rules = vec![ChannelResolutionRuleRecord {
            id: "rule_01".to_string(),
            policy_set_id: "policy_set_01".to_string(),
            priority: 25,
            is_active: true,
            action_channel_id: "channel_01".to_string(),
            definition: ChannelResolutionRuleDefinitionRecord {
                predicates: vec![],
                action: ChannelResolutionActionRecord::ResolveToChannel {
                    channel_id: "channel_01".to_string(),
                },
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }];
        let state_with_rules = policy_rule_create_form_state(&existing_rules, &channels);
        assert_eq!(state_with_rules.priority, 35);
    }

    #[test]
    fn policy_rule_edit_form_state_prefills_predicates_and_action_channel() {
        let rule = ChannelResolutionRuleRecord {
            id: "rule_01".to_string(),
            policy_set_id: "policy_set_01".to_string(),
            priority: 30,
            is_active: false,
            action_channel_id: "channel_01".to_string(),
            definition: ChannelResolutionRuleDefinitionRecord {
                predicates: vec![
                    ChannelResolutionPredicateRecord::HostEquals("shop.example.test".to_string()),
                    ChannelResolutionPredicateRecord::OAuthAppEquals(
                        "550e8400-e29b-41d4-a716-446655440000".to_string(),
                    ),
                    ChannelResolutionPredicateRecord::SurfaceIs("http".to_string()),
                    ChannelResolutionPredicateRecord::LocaleEquals("ru-by".to_string()),
                ],
                action: ChannelResolutionActionRecord::ResolveToChannel {
                    channel_id: "channel_01".to_string(),
                },
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let channels = vec![ChannelDetail {
            channel: ChannelRecord {
                id: "channel_01".to_string(),
                tenant_id: "tenant_01".to_string(),
                slug: "web".to_string(),
                name: "Web".to_string(),
                is_active: true,
                is_default: true,
                settings: serde_json::json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            targets: Vec::new(),
            module_bindings: Vec::new(),
            oauth_apps: Vec::new(),
        }];

        let form_state = policy_rule_edit_form_state(&rule, &channels);

        assert_eq!(form_state.priority, 30);
        assert!(!form_state.is_active);
        assert_eq!(form_state.action_channel_id, "channel_01");
        assert_eq!(form_state.host_equals, "shop.example.test");
        assert_eq!(
            form_state.oauth_app_id,
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(form_state.surface, "http");
        assert_eq!(form_state.locale, "ru-by");
    }
}
