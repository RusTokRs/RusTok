use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use fly_ui::{
    CapabilityState, ContributionAssemblyResult, ContributionAssemblySeverity,
    EditorCapabilityPolicy, EditorProviderState,
};

/// Builds the Pages editor policy from the effective RBAC permission snapshot
/// and the actual contribution assembly health. Authoritative callers must
/// supply the snapshot verified by the backend auth service; an empty or
/// unrecognized snapshot fails closed to read-only.
pub fn pages_editor_capability_policy(
    permissions: &[String],
    assembly: &ContributionAssemblyResult,
) -> EditorCapabilityPolicy {
    EditorCapabilityPolicy {
        requested: CapabilityState::full(),
        tenant: CapabilityState::full(),
        permissions: pages_editor_permissions_from_effective(permissions),
        provider_state: pages_editor_provider_state(assembly),
        allow_publish_when_degraded: false,
    }
}

pub fn pages_editor_capability_policy_from_effective(
    permissions: &[String],
) -> EditorCapabilityPolicy {
    let assembly = build_pages_admin_contribution_registry(&pages_admin_contribution_policy());
    pages_editor_capability_policy(permissions, &assembly)
}

/// Whether an effective permission snapshot grants `required`
/// (`"{resource}:{action}"`, e.g. `pages:update`). Mirrors
/// `rustok_api::has_effective_permission`: an exact match or the resource's
/// `manage` grant suffices; no other implication exists. Malformed entries
/// are ignored rather than guessed from a role string.
pub fn effective_permissions_grant(effective: &[String], required: &str) -> bool {
    let required = required.trim();
    let Some((resource, _action)) = required.split_once(':') else {
        return false;
    };
    effective.iter().any(|granted| {
        let granted = granted.trim();
        if granted.eq_ignore_ascii_case(required) {
            return true;
        }
        granted
            .split_once(':')
            .is_some_and(|(granted_resource, granted_action)| {
                granted_resource.trim().eq_ignore_ascii_case(resource.trim())
                    && matches!(granted_action.trim(), "manage" | "*")
            })
    })
}

/// Maps the effective permission snapshot onto editor capabilities:
/// `pages:manage` grants everything; `pages:update` grants authoring
/// (edit/properties/assets/…); `pages:publish` grants publishing only.
/// Users without Pages grants stay read-only. This matches the port
/// authorization model, where the Properties capability requires
/// `pages:update` and Publish requires `pages:publish`.
pub fn pages_editor_permissions_from_effective(permissions: &[String]) -> CapabilityState {
    let can_update = effective_permissions_grant(permissions, "pages:update");
    let can_publish = effective_permissions_grant(permissions, "pages:publish");
    match (can_update, can_publish) {
        (true, true) => CapabilityState::full(),
        (true, false) => CapabilityState {
            publish: false,
            ..CapabilityState::full()
        },
        (false, true) => CapabilityState {
            publish: true,
            ..CapabilityState::read_only()
        },
        (false, false) => CapabilityState::read_only(),
    }
}

/// Whether the principal may restore a journaled body revision.
///
/// Restoring a revision is a save, so it needs `pages:update`. The journal
/// itself is readable with `pages:publish` as well, and a principal that
/// reads it that way is not offered an action the server would refuse. The
/// server stays the authority: it also applies the ownership scope of the
/// grant.
pub fn pages_can_restore_body_revision(permissions: &[String]) -> bool {
    effective_permissions_grant(permissions, "pages:update")
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
    fn restoring_a_body_revision_needs_the_save_grant() {
        let grants = |granted: &[&str]| {
            pages_can_restore_body_revision(
                &granted
                    .iter()
                    .map(|grant| grant.to_string())
                    .collect::<Vec<_>>(),
            )
        };
        assert!(grants(&["pages:update"]));
        assert!(grants(&["pages:manage"]));
        // The journal is readable with the publish grant; restoring is not.
        assert!(!grants(&["pages:publish"]));
        assert!(!grants(&["pages:read", "pages:list"]));
        assert!(!grants(&[]));
    }

    #[test]
    fn manage_grant_receives_full_editor_access() {
        assert_eq!(
            pages_editor_permissions_from_effective(&["pages:manage".to_string()]),
            CapabilityState::full()
        );
    }

    #[test]
    fn update_and_publish_grants_together_grant_full_access() {
        assert_eq!(
            pages_editor_permissions_from_effective(&[
                "pages:update".to_string(),
                "pages:publish".to_string(),
            ]),
            CapabilityState::full()
        );
    }

    #[test]
    fn update_grant_authors_but_cannot_publish() {
        let permissions =
            pages_editor_permissions_from_effective(&["pages:update".to_string()]);
        assert!(permissions.edit);
        assert!(permissions.properties);
        assert!(permissions.assets);
        assert!(!permissions.publish);
    }

    #[test]
    fn publish_grant_without_update_only_publishes() {
        let permissions =
            pages_editor_permissions_from_effective(&["pages:publish".to_string()]);
        assert!(!permissions.edit);
        assert!(!permissions.properties);
        assert!(!permissions.assets);
        assert!(permissions.publish);
    }

    #[test]
    fn unrelated_missing_and_malformed_permissions_fail_closed() {
        for permissions in [
            vec![],
            vec!["products:update".to_string()],
            vec!["users:manage".to_string()],
            vec!["pages".to_string()],
            vec!["".to_string()],
        ] {
            assert_eq!(
                pages_editor_permissions_from_effective(&permissions),
                CapabilityState::read_only()
            );
        }
    }

    #[test]
    fn manage_grant_satisfies_any_required_permission_for_its_resource() {
        let effective = vec![
            "forum_topics:manage".to_string(),
            "pages:update".to_string(),
        ];
        assert!(effective_permissions_grant(&effective, "forum_topics:moderate"));
        assert!(!effective_permissions_grant(&effective, "blog_posts:update"));
        assert!(!effective_permissions_grant(&effective, "pages:publish"));
        assert!(effective_permissions_grant(
            &["  pages:manage  ".to_string()],
            "pages:publish"
        ));
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
    fn healthy_pages_registry_preserves_effective_permissions() {
        let policy =
            pages_editor_capability_policy_from_effective(&["pages:update".to_string()]);
        let evaluation = policy.evaluate_detailed();
        assert_eq!(evaluation.provider_state, EditorProviderState::Healthy);
        assert!(evaluation.effective.edit);
        assert!(!evaluation.effective.publish);
    }
}
