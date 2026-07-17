use crate::CapabilityState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum EditorCapability {
    Edit,
    DragDrop,
    Properties,
    Styles,
    Assets,
    Clipboard,
    History,
    Publish,
}

impl EditorCapability {
    pub const ALL: [Self; 8] = [
        Self::Edit,
        Self::DragDrop,
        Self::Properties,
        Self::Styles,
        Self::Assets,
        Self::Clipboard,
        Self::History,
        Self::Publish,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::DragDrop => "drag_drop",
            Self::Properties => "properties",
            Self::Styles => "styles",
            Self::Assets => "assets",
            Self::Clipboard => "clipboard",
            Self::History => "history",
            Self::Publish => "publish",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorProviderState {
    Healthy,
    Degraded,
    Unavailable,
}

impl EditorProviderState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }
}

impl Default for EditorProviderState {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Framework-neutral capability evaluation for a concrete editor surface.
///
/// The effective profile is the intersection of requested product features, tenant policy and
/// user permissions. Provider health can then downgrade publish or force a read-only surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorCapabilityPolicy {
    pub requested: CapabilityState,
    pub tenant: CapabilityState,
    pub permissions: CapabilityState,
    #[serde(default)]
    pub provider_state: EditorProviderState,
    #[serde(default)]
    pub allow_publish_when_degraded: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditorCapabilityEvaluation {
    pub requested: CapabilityState,
    pub tenant: CapabilityState,
    pub permissions: CapabilityState,
    pub provider_state: EditorProviderState,
    pub allow_publish_when_degraded: bool,
    pub effective: CapabilityState,
}

impl EditorCapabilityEvaluation {
    pub const fn allows(self, capability: EditorCapability) -> bool {
        self.effective.allows(capability)
    }

    pub const fn requested_allows(self, capability: EditorCapability) -> bool {
        self.requested.allows(capability)
    }

    pub const fn tenant_allows(self, capability: EditorCapability) -> bool {
        self.tenant.allows(capability)
    }

    pub const fn permission_allows(self, capability: EditorCapability) -> bool {
        self.permissions.allows(capability)
    }
}

impl Default for EditorCapabilityPolicy {
    fn default() -> Self {
        Self {
            requested: CapabilityState::full(),
            tenant: CapabilityState::full(),
            permissions: CapabilityState::full(),
            provider_state: EditorProviderState::Healthy,
            allow_publish_when_degraded: false,
        }
    }
}

impl EditorCapabilityPolicy {
    pub fn evaluate(self) -> CapabilityState {
        self.evaluate_detailed().effective
    }

    pub fn evaluate_detailed(self) -> EditorCapabilityEvaluation {
        let mut effective = self
            .requested
            .intersection(self.tenant)
            .intersection(self.permissions);
        match self.provider_state {
            EditorProviderState::Healthy => {}
            EditorProviderState::Degraded => {
                if !self.allow_publish_when_degraded {
                    effective.publish = false;
                }
            }
            EditorProviderState::Unavailable => effective = CapabilityState::read_only(),
        }
        EditorCapabilityEvaluation {
            requested: self.requested,
            tenant: self.tenant,
            permissions: self.permissions,
            provider_state: self.provider_state,
            allow_publish_when_degraded: self.allow_publish_when_degraded,
            effective: effective.normalized(),
        }
    }
}

impl CapabilityState {
    pub const fn intersection(self, other: Self) -> Self {
        Self {
            edit: self.edit && other.edit,
            drag_drop: self.drag_drop && other.drag_drop,
            properties: self.properties && other.properties,
            styles: self.styles && other.styles,
            assets: self.assets && other.assets,
            clipboard: self.clipboard && other.clipboard,
            history: self.history && other.history,
            publish: self.publish && other.publish,
        }
    }

    /// Mutating sub-capabilities cannot survive when the general edit capability is absent.
    /// Publish remains independent so a reviewer may publish an unchanged draft when policy allows.
    pub const fn normalized(mut self) -> Self {
        if !self.edit {
            self.drag_drop = false;
            self.properties = false;
            self.styles = false;
            self.assets = false;
            self.clipboard = false;
            self.history = false;
        }
        self
    }

    pub const fn allows(self, capability: EditorCapability) -> bool {
        match capability {
            EditorCapability::Edit => self.edit,
            EditorCapability::DragDrop => self.drag_drop,
            EditorCapability::Properties => self.properties,
            EditorCapability::Styles => self.styles,
            EditorCapability::Assets => self.assets,
            EditorCapability::Clipboard => self.clipboard,
            EditorCapability::History => self.history,
            EditorCapability::Publish => self.publish,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_intersects_tenant_and_permission_capabilities() {
        let evaluation = EditorCapabilityPolicy {
            tenant: CapabilityState {
                assets: false,
                ..CapabilityState::full()
            },
            permissions: CapabilityState {
                publish: false,
                ..CapabilityState::full()
            },
            ..EditorCapabilityPolicy::default()
        }
        .evaluate_detailed();
        assert!(!evaluation.effective.assets);
        assert!(!evaluation.effective.publish);
        assert!(evaluation.effective.drag_drop);
        assert!(!evaluation.tenant_allows(EditorCapability::Assets));
        assert!(!evaluation.permission_allows(EditorCapability::Publish));
    }

    #[test]
    fn degraded_provider_disables_publish_without_destroying_draft_editing() {
        let evaluation = EditorCapabilityPolicy {
            provider_state: EditorProviderState::Degraded,
            ..EditorCapabilityPolicy::default()
        }
        .evaluate_detailed();
        assert!(evaluation.effective.edit);
        assert!(evaluation.effective.history);
        assert!(!evaluation.effective.publish);
        assert_eq!(evaluation.provider_state.as_str(), "degraded");
    }

    #[test]
    fn unavailable_provider_forces_read_only() {
        assert_eq!(
            EditorCapabilityPolicy {
                provider_state: EditorProviderState::Unavailable,
                ..EditorCapabilityPolicy::default()
            }
            .evaluate(),
            CapabilityState::read_only()
        );
    }

    #[test]
    fn edit_denial_removes_mutating_sub_capabilities_but_can_preserve_publish() {
        let effective = EditorCapabilityPolicy {
            permissions: CapabilityState {
                edit: false,
                publish: true,
                ..CapabilityState::full()
            },
            ..EditorCapabilityPolicy::default()
        }
        .evaluate();
        assert!(!effective.edit);
        assert!(!effective.drag_drop);
        assert!(!effective.history);
        assert!(effective.publish);
    }

    #[test]
    fn capability_enum_is_stable_and_exhaustive() {
        assert_eq!(EditorCapability::ALL.len(), 8);
        assert!(CapabilityState::full().allows(EditorCapability::Styles));
        assert!(!CapabilityState::read_only().allows(EditorCapability::Styles));
        assert_eq!(EditorCapability::DragDrop.as_str(), "drag_drop");
    }
}
