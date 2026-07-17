use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, ContributionAssemblySeverity,
    EditorCapabilityPolicy, EditorProviderState,
};

/// Builds the Pages editor policy from a canonical RusTok role and the actual contribution assembly
/// health. Authoritative callers must supply a role verified by the backend auth service. Unknown
/// roles fail closed; no local permission identifiers are invented here.
pub fn pages_editor_capability_policy(
    role: Option<&str>,
    assembly: &ContributionAssemblyResult,
) -> EditorCapabilityPolicy {
    EditorCapabilityPolicy {
        requested: CapabilityState::full(),
        tenant: CapabilityState::full(),
        permissions: pages_editor_permissions_for_role(role),
        provider_state: pages_editor_provider_state(assembly),
        allow_publish_when_degraded: false,
    }
}

pub fn pages_editor_capability_policy_for_role(role: Option<&str>) -> EditorCapabilityPolicy {
    let assembly = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
    pages_editor_capability_policy(role, &assembly)
}

pub fn pages_editor_permissions_for_role(role: Option<&str>) -> CapabilityState {
    let role = role
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(str::to_ascii_lowercase);
    match role.as_deref() {
        Some("super_admin") | Some("admin") => CapabilityState::full(),
        Some("manager") => CapabilityState {
            publish: false,
            ..CapabilityState::full()
        },
        Some("customer") | None | Some(_) => CapabilityState::read_only(),
    }
}

pub fn pages_editor_provider_state(
    assembly: &ContributionAssemblyResult,
) -> EditorProviderState {
    if assembly
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ContributionAssemblySeverity::Error)
    {
        EditorProviderState::Unavailable
    } else if assembly
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ContributionAssemblySeverity::Warning)
    {
        EditorProviderState::Degraded
    } else {
        EditorProviderState::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::{ContributionAssemblyDiagnostic, ContributionAssemblyResult};

    #[test]
    fn canonical_admin_roles_receive_full_editor_access() {
        for role in ["super_admin", "admin", " ADMIN "] {
            assert_eq!(
                pages_editor_permissions_for_role(Some(role)),
                CapabilityState::full()
            );
        }
    }

    #[test]
    fn manager_can_author_but_cannot_publish() {
        let permissions = pages_editor_permissions_for_role(Some("manager"));
        assert!(permissions.edit);
        assert!(permissions.properties);
        assert!(permissions.assets);
        assert!(!permissions.publish);
    }

    #[test]
    fn customer_unknown_and_unauthenticated_roles_fail_closed() {
        for role in [Some("customer"), Some("future_role"), None] {
            assert_eq!(
                pages_editor_permissions_for_role(role),
                CapabilityState::read_only()
            );
        }
    }

    #[test]
    fn contribution_errors_force_unavailable_provider_state() {
        let assembly = ContributionAssemblyResult {
            diagnostics: vec![ContributionAssemblyDiagnostic {
                severity: ContributionAssemblySeverity::Error,
                code: "broken_provider".to_string(),
                module_id: Some("pages".to_string()),
                contribution_id: None,
                message: "provider failed".to_string(),
            }],
            ..ContributionAssemblyResult::default()
        };
        assert_eq!(
            pages_editor_provider_state(&assembly),
            EditorProviderState::Unavailable
        );
    }

    #[test]
    fn healthy_pages_registry_preserves_role_permissions() {
        let policy = pages_editor_capability_policy_for_role(Some("manager"));
        let evaluation = policy.evaluate_detailed();
        assert_eq!(evaluation.provider_state, EditorProviderState::Healthy);
        assert!(evaluation.effective.edit);
        assert!(!evaluation.effective.publish);
    }
}
