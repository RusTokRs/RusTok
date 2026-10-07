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

/// Fly editor capabilities for a canonical RusTok role.
///
/// In Fly, the `publish` capability means "persist the edited document to the host store". For
/// Pages that is a draft save (`savePageDocument`, `pages:update`), not a page publication, so
/// every role that may update pages gets it. Making a page public is a separate Pages lifecycle
/// action gated by [`pages_lifecycle_permissions_for_role`].
pub fn pages_editor_permissions_for_role(role: Option<&str>) -> CapabilityState {
    if pages_lifecycle_permissions_for_role(role).save_draft {
        CapabilityState::full()
    } else {
        CapabilityState::read_only()
    }
}

/// Page lifecycle actions a role may perform, mirroring the canonical RBAC role tables in
/// `rustok-core` (`pages:update`, `pages:publish`, `pages:delete`). The server re-checks every
/// action; this only keeps the UI honest about what will succeed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PagesLifecyclePermissions {
    pub create: bool,
    pub save_draft: bool,
    pub publish: bool,
    pub unpublish: bool,
    pub delete: bool,
}

impl PagesLifecyclePermissions {
    pub const fn full() -> Self {
        Self {
            create: true,
            save_draft: true,
            publish: true,
            unpublish: true,
            delete: true,
        }
    }

    pub const fn none() -> Self {
        Self {
            create: false,
            save_draft: false,
            publish: false,
            unpublish: false,
            delete: false,
        }
    }
}

pub fn pages_lifecycle_permissions_for_role(role: Option<&str>) -> PagesLifecyclePermissions {
    let role = role
        .map(str::trim)
        .filter(|role| !role.is_empty())
        .map(str::to_ascii_lowercase);
    match role.as_deref() {
        Some("super_admin") | Some("admin") => PagesLifecyclePermissions::full(),
        // Managers author and remove drafts but cannot make pages public.
        Some("manager") => PagesLifecyclePermissions {
            create: true,
            save_draft: true,
            publish: false,
            unpublish: false,
            delete: true,
        },
        Some("customer") | None | Some(_) => PagesLifecyclePermissions::none(),
    }
}

pub fn pages_editor_provider_state(assembly: &ContributionAssemblyResult) -> EditorProviderState {
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
    fn manager_can_author_and_save_drafts_but_cannot_publish_pages() {
        let permissions = pages_editor_permissions_for_role(Some("manager"));
        assert!(permissions.edit);
        assert!(permissions.properties);
        assert!(permissions.assets);
        // Fly `publish` persists the draft document; managers must be able to save their work.
        assert!(permissions.publish);

        let lifecycle = pages_lifecycle_permissions_for_role(Some("manager"));
        assert!(lifecycle.save_draft);
        assert!(lifecycle.create);
        assert!(lifecycle.delete);
        assert!(!lifecycle.publish);
        assert!(!lifecycle.unpublish);
    }

    #[test]
    fn lifecycle_permissions_fail_closed_for_unknown_roles() {
        for role in [Some("customer"), Some("future_role"), Some(""), None] {
            assert_eq!(
                pages_lifecycle_permissions_for_role(role),
                PagesLifecyclePermissions::none()
            );
        }
        assert_eq!(
            pages_lifecycle_permissions_for_role(Some(" Admin ")),
            PagesLifecyclePermissions::full()
        );
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
        assert!(evaluation.effective.publish);
    }
}
