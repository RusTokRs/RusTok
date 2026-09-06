use crate::services::permission_policy::{
    DeniedReasonKind, PermissionCheckOutcome, check_all_permissions, check_any_permission,
    check_permission,
};
use rustok_api::Permission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionEvaluation {
    pub allowed: bool,
    pub missing_permissions: Vec<Permission>,
    pub denied_reason: Option<(DeniedReasonKind, String)>,
}

impl PermissionEvaluation {
    fn from_outcome(
        outcome: PermissionCheckOutcome,
        user_permissions: &[Permission],
    ) -> PermissionEvaluation {
        let denied_reason = outcome.denied_reason(user_permissions);

        PermissionEvaluation {
            allowed: outcome.allowed,
            missing_permissions: outcome.missing_permissions,
            denied_reason,
        }
    }
}

pub fn evaluate_single_permission(
    user_permissions: &[Permission],
    required_permission: &Permission,
) -> PermissionEvaluation {
    PermissionEvaluation::from_outcome(
        check_permission(user_permissions, required_permission),
        user_permissions,
    )
}

pub fn evaluate_any_permission(
    user_permissions: &[Permission],
    required_permissions: &[Permission],
) -> PermissionEvaluation {
    PermissionEvaluation::from_outcome(
        check_any_permission(user_permissions, required_permissions),
        user_permissions,
    )
}

pub fn evaluate_all_permissions(
    user_permissions: &[Permission],
    required_permissions: &[Permission],
) -> PermissionEvaluation {
    PermissionEvaluation::from_outcome(
        check_all_permissions(user_permissions, required_permissions),
        user_permissions,
    )
}

#[cfg(test)]
mod tests {
    use super::{evaluate_all_permissions, evaluate_any_permission, evaluate_single_permission};
    use crate::DeniedReasonKind;
    use rustok_api::Permission;

    #[test]
    fn evaluate_single_permission_returns_denied_reason() {
        let evaluation = evaluate_single_permission(&[], &Permission::USERS_READ);

        assert!(!evaluation.allowed);
        assert_eq!(evaluation.missing_permissions, vec![Permission::USERS_READ]);
        assert_eq!(
            evaluation.denied_reason,
            Some((
                DeniedReasonKind::NoPermissionsResolved,
                "no_permissions_resolved".to_string(),
            )),
        );
    }

    #[test]
    fn evaluate_any_permission_returns_allowed_without_denied_reason() {
        let evaluation = evaluate_any_permission(
            &[Permission::USERS_MANAGE],
            &[Permission::USERS_READ, Permission::USERS_UPDATE],
        );

        assert!(evaluation.allowed);
        assert!(evaluation.missing_permissions.is_empty());
        assert!(evaluation.denied_reason.is_none());
    }

    #[test]
    fn evaluate_all_permissions_reports_missing_permissions() {
        let evaluation = evaluate_all_permissions(
            &[Permission::USERS_READ],
            &[Permission::USERS_READ, Permission::USERS_UPDATE],
        );

        assert!(!evaluation.allowed);
        assert_eq!(
            evaluation.missing_permissions,
            vec![Permission::USERS_UPDATE]
        );
        assert_eq!(
            evaluation.denied_reason.as_ref().map(|(kind, _)| *kind),
            Some(DeniedReasonKind::MissingPermissions),
        );
    }
}
