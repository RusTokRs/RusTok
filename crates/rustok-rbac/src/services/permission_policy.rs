use crate::services::permission_normalization::normalize_permissions;
use rustok_api::{Action, Permission};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeniedReasonKind {
    NoPermissionsResolved,
    MissingPermissions,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionCheckOutcome {
    pub allowed: bool,
    pub missing_permissions: Vec<Permission>,
}

impl PermissionCheckOutcome {
    pub fn denied_reason(
        &self,
        user_permissions: &[Permission],
    ) -> Option<(DeniedReasonKind, String)> {
        if self.allowed {
            return None;
        }

        Some(denied_reason_for_denial(
            user_permissions,
            &self.missing_permissions,
        ))
    }
}

pub fn has_effective_permission_in_set(
    user_permissions: &[Permission],
    required_permission: &Permission,
) -> bool {
    user_permissions.contains(required_permission)
        || user_permissions.contains(&Permission::new(
            required_permission.resource,
            Action::Manage,
        ))
}

pub fn missing_permissions(
    user_permissions: &[Permission],
    required_permissions: &[Permission],
) -> Vec<Permission> {
    normalize_permissions(
        required_permissions
            .iter()
            .copied()
            .filter(|permission| !has_effective_permission_in_set(user_permissions, permission))
            .collect(),
    )
}

pub fn check_permission(
    user_permissions: &[Permission],
    required_permission: &Permission,
) -> PermissionCheckOutcome {
    let allowed = has_effective_permission_in_set(user_permissions, required_permission);
    let missing_permissions = if allowed {
        Vec::new()
    } else {
        vec![*required_permission]
    };

    PermissionCheckOutcome {
        allowed,
        missing_permissions,
    }
}

pub fn check_any_permission(
    user_permissions: &[Permission],
    required_permissions: &[Permission],
) -> PermissionCheckOutcome {
    if required_permissions.is_empty() {
        return PermissionCheckOutcome {
            allowed: true,
            missing_permissions: Vec::new(),
        };
    }

    let allowed = required_permissions
        .iter()
        .any(|permission| has_effective_permission_in_set(user_permissions, permission));

    let missing_permissions = if allowed {
        Vec::new()
    } else {
        normalize_permissions(required_permissions.to_vec())
    };

    PermissionCheckOutcome {
        allowed,
        missing_permissions,
    }
}

pub fn check_all_permissions(
    user_permissions: &[Permission],
    required_permissions: &[Permission],
) -> PermissionCheckOutcome {
    if required_permissions.is_empty() {
        return PermissionCheckOutcome {
            allowed: true,
            missing_permissions: Vec::new(),
        };
    }

    let missing_permissions = missing_permissions(user_permissions, required_permissions);
    PermissionCheckOutcome {
        allowed: missing_permissions.is_empty(),
        missing_permissions,
    }
}

pub fn denied_reason_for_denial(
    user_permissions: &[Permission],
    missing_permissions: &[Permission],
) -> (DeniedReasonKind, String) {
    if user_permissions.is_empty() {
        return (
            DeniedReasonKind::NoPermissionsResolved,
            "no_permissions_resolved".to_string(),
        );
    }

    if missing_permissions.is_empty() {
        return (DeniedReasonKind::Unknown, "unknown".to_string());
    }

    let mut reason = String::from("missing_permissions:");
    for (index, permission) in missing_permissions.iter().enumerate() {
        if index > 0 {
            reason.push(',');
        }
        let _ = write!(&mut reason, "{}", permission);
    }

    (DeniedReasonKind::MissingPermissions, reason)
}

#[cfg(test)]
mod tests {
    use super::{
        DeniedReasonKind, check_all_permissions, check_any_permission, check_permission,
        denied_reason_for_denial, has_effective_permission_in_set, missing_permissions,
    };
    use rustok_api::{Action, Permission, Resource};

    #[test]
    fn effective_permission_supports_manage_wildcard() {
        let permissions = vec![Permission::new(Resource::Users, Action::Manage)];

        assert!(has_effective_permission_in_set(
            &permissions,
            &Permission::USERS_UPDATE,
        ));
    }

    #[test]
    fn missing_permissions_respects_manage_wildcard() {
        let permissions = vec![Permission::new(Resource::Users, Action::Manage)];
        let required = vec![Permission::USERS_READ, Permission::USERS_UPDATE];

        assert!(missing_permissions(&permissions, &required).is_empty());
    }

    #[test]
    fn check_permission_reports_missing_for_single_permission() {
        let outcome = check_permission(&[Permission::USERS_READ], &Permission::USERS_UPDATE);

        assert!(!outcome.allowed);
        assert_eq!(outcome.missing_permissions, vec![Permission::USERS_UPDATE]);
    }

    #[test]
    fn check_any_permission_returns_normalized_missing_permissions() {
        let outcome = check_any_permission(
            &[],
            &[
                Permission::USERS_UPDATE,
                Permission::USERS_READ,
                Permission::USERS_UPDATE,
            ],
        );

        assert!(!outcome.allowed);
        assert_eq!(
            outcome.missing_permissions,
            vec![Permission::USERS_READ, Permission::USERS_UPDATE]
        );
    }

    #[test]
    fn check_any_permission_accepts_empty_requirements() {
        let outcome = check_any_permission(&[], &[]);
        assert!(outcome.allowed);
        assert!(outcome.missing_permissions.is_empty());
    }

    #[test]
    fn check_all_permissions_accepts_empty_requirements() {
        let outcome = check_all_permissions(&[], &[]);
        assert!(outcome.allowed);
        assert!(outcome.missing_permissions.is_empty());
    }

    #[test]
    fn denied_reason_reports_no_permissions() {
        let (reason_kind, reason) = denied_reason_for_denial(&[], &[Permission::USERS_READ]);

        assert_eq!(reason_kind, DeniedReasonKind::NoPermissionsResolved);
        assert_eq!(reason, "no_permissions_resolved");
    }

    #[test]
    fn denied_reason_reports_missing_permissions() {
        let (reason_kind, reason) =
            denied_reason_for_denial(&[Permission::USERS_READ], &[Permission::USERS_UPDATE]);

        assert_eq!(reason_kind, DeniedReasonKind::MissingPermissions);
        assert!(reason.starts_with("missing_permissions:"));
    }
}
