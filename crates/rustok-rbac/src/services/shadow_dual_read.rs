use rustok_core::UserRole;

use super::shadow_decision::{compare_shadow_decision, ShadowCheck, ShadowDecision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualReadOutcome {
    Skipped,
    Compared(ShadowDecision),
}

pub fn evaluate_dual_read(
    legacy_role: Option<&UserRole>,
    shadow_check: ShadowCheck<'_>,
    relation_allowed: bool,
) -> DualReadOutcome {
    match legacy_role {
        Some(legacy_role) => DualReadOutcome::Compared(compare_shadow_decision(
            legacy_role,
            shadow_check,
            relation_allowed,
        )),
        None => DualReadOutcome::Skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::{evaluate_dual_read, DualReadOutcome};
    use crate::services::shadow_decision::{ShadowCheck, ShadowDecision};
    use rustok_core::{Action, Permission, Resource, UserRole};

    fn permission(resource: Resource, action: Action) -> Permission {
        Permission::new(resource, action)
    }

    #[test]
    fn returns_compared_when_legacy_role_present() {
        let required = permission(Resource::BlogPosts, Action::Read);

        let outcome = evaluate_dual_read(
            Some(&UserRole::Manager),
            ShadowCheck::Single(&required),
            true,
        );

        assert_eq!(
            outcome,
            DualReadOutcome::Compared(ShadowDecision {
                legacy_allowed: true,
                relation_allowed: true,
            })
        );
    }

    #[test]
    fn returns_skipped_when_legacy_role_missing() {
        let required = permission(Resource::Users, Action::Read);

        let outcome = evaluate_dual_read(None, ShadowCheck::Single(&required), true);
        assert_eq!(outcome, DualReadOutcome::Skipped);
    }

    #[test]
    fn returns_matched_when_relation_equals_legacy() {
        let required = permission(Resource::BlogPosts, Action::Read);

        let outcome = evaluate_dual_read(
            Some(&UserRole::Manager),
            ShadowCheck::Single(&required),
            true,
        );

        assert_eq!(
            outcome,
            DualReadOutcome::Compared(ShadowDecision {
                legacy_allowed: true,
                relation_allowed: true,
            })
        );
    }

    #[test]
    fn returns_mismatch_when_relation_differs_from_legacy() {
        let required = permission(Resource::Users, Action::Delete);

        let outcome = evaluate_dual_read(
            Some(&UserRole::Manager),
            ShadowCheck::Single(&required),
            true,
        );

        assert_eq!(
            outcome,
            DualReadOutcome::Compared(ShadowDecision {
                legacy_allowed: false,
                relation_allowed: true,
            })
        );
    }
}
