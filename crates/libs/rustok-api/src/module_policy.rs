//! Browser-safe effective module-policy projections.
//!
//! The modules owner keeps the full policy evidence private. These values
//! carry the revisioned availability decision and stable denial taxonomy that
//! operator clients need without exposing resolver internals or capability
//! grant contents.

use serde::{Deserialize, Serialize};

/// One browser-safe effective-policy snapshot for the authenticated tenant.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleEffectivePolicyView {
    #[serde(rename = "policyRevision")]
    pub policy_revision: String,
    pub decisions: Vec<ModuleEffectivePolicyDecisionView>,
}

impl ModuleEffectivePolicyView {
    /// Returns the enabled module identities from this exact owner decision.
    ///
    /// Decisions are emitted in canonical module-slug order by the owner.
    pub fn enabled_module_slugs(&self) -> Vec<String> {
        self.decisions
            .iter()
            .filter(|decision| decision.enabled)
            .map(|decision| decision.module_slug.clone())
            .collect()
    }
}

/// Browser-safe availability result for one module under a policy revision.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ModuleEffectivePolicyDecisionView {
    #[serde(rename = "moduleSlug")]
    pub module_slug: String,
    pub enabled: bool,
    #[serde(rename = "policyRevision")]
    pub policy_revision: String,
    #[serde(rename = "denialReasons")]
    pub denial_reasons: Vec<ModuleEffectivePolicyDenialReasonView>,
}

/// Stable, redacted explanation for why a module is unavailable.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ModuleEffectivePolicyDenialReasonView {
    UnknownModule,
    NotSelected,
    TenantDisabled,
    ArtifactInstallationUnavailable,
    CapabilityPolicyUnavailable,
    ExecutorUnavailable,
    DependencyUnavailable {
        #[serde(rename = "moduleSlug")]
        module_slug: String,
    },
    CoRequisiteUnavailable {
        #[serde(rename = "moduleSlug")]
        module_slug: String,
    },
    CoRequisiteVersionMismatch {
        #[serde(rename = "moduleSlug")]
        module_slug: String,
    },
    RegistryReleaseUnavailable,
    SecurityStateUnavailable,
    Quarantined,
    Revoked,
    ChannelInactive,
    ChannelBindingUnavailable,
    ChannelDisabled,
    MaintenanceActive,
}

impl ModuleEffectivePolicyDenialReasonView {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownModule => "unknown_module",
            Self::NotSelected => "not_selected",
            Self::TenantDisabled => "tenant_disabled",
            Self::ArtifactInstallationUnavailable => "artifact_installation_unavailable",
            Self::CapabilityPolicyUnavailable => "capability_policy_unavailable",
            Self::ExecutorUnavailable => "executor_unavailable",
            Self::DependencyUnavailable { .. } => "dependency_unavailable",
            Self::CoRequisiteUnavailable { .. } => "co_requisite_unavailable",
            Self::CoRequisiteVersionMismatch { .. } => "co_requisite_version_mismatch",
            Self::RegistryReleaseUnavailable => "registry_release_unavailable",
            Self::SecurityStateUnavailable => "security_state_unavailable",
            Self::Quarantined => "quarantined",
            Self::Revoked => "revoked",
            Self::ChannelInactive => "channel_inactive",
            Self::ChannelBindingUnavailable => "channel_binding_unavailable",
            Self::ChannelDisabled => "channel_disabled",
            Self::MaintenanceActive => "maintenance_active",
        }
    }

    pub fn related_module_slug(&self) -> Option<&str> {
        match self {
            Self::DependencyUnavailable { module_slug }
            | Self::CoRequisiteUnavailable { module_slug }
            | Self::CoRequisiteVersionMismatch { module_slug } => Some(module_slug),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ModuleEffectivePolicyDecisionView, ModuleEffectivePolicyDenialReasonView,
        ModuleEffectivePolicyView,
    };

    #[test]
    fn policy_view_uses_the_graphql_field_contract() {
        let view = ModuleEffectivePolicyView {
            policy_revision: "sha256:policy".to_string(),
            decisions: vec![ModuleEffectivePolicyDecisionView {
                module_slug: "product".to_string(),
                enabled: false,
                policy_revision: "sha256:policy".to_string(),
                denial_reasons: vec![
                    ModuleEffectivePolicyDenialReasonView::CoRequisiteUnavailable {
                        module_slug: "inventory".to_string(),
                    },
                ],
            }],
        };

        let encoded = serde_json::to_value(&view).expect("policy view serializes");
        assert_eq!(encoded["policyRevision"], "sha256:policy");
        assert_eq!(encoded["decisions"][0]["moduleSlug"], "product");
        assert_eq!(
            encoded["decisions"][0]["denialReasons"][0]["kind"],
            "co_requisite_unavailable"
        );
        assert_eq!(
            encoded["decisions"][0]["denialReasons"][0]["moduleSlug"],
            "inventory"
        );
        assert!(view.enabled_module_slugs().is_empty());
    }

    #[test]
    fn denial_reason_exposes_a_stable_code_and_related_module() {
        let reason = ModuleEffectivePolicyDenialReasonView::DependencyUnavailable {
            module_slug: "taxonomy".to_string(),
        };

        assert_eq!(reason.code(), "dependency_unavailable");
        assert_eq!(reason.related_module_slug(), Some("taxonomy"));
    }
}
