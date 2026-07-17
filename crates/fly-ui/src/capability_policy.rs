use crate::CapabilityState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorProviderState {
    Healthy,
    Degraded,
    Unavailable,
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
            EditorProviderState::Unavailable => return CapabilityState::read_only(),
        }
        effective.normalized()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_intersects_tenant_and_permission_capabilities() {
        let effective = EditorCapabilityPolicy {
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
        .evaluate();
        assert!(!effective.assets);
        assert!(!effective.publish);
        assert!(effective.drag_drop);
    }

    #[test]
    fn degraded_provider_disables_publish_without_destroying_draft_editing() {
        let effective = EditorCapabilityPolicy {
            provider_state: EditorProviderState::Degraded,
            ..EditorCapabilityPolicy::default()
        }
        .evaluate();
        assert!(effective.edit);
        assert!(effective.history);
        assert!(!effective.publish);
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
}
