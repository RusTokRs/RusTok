//! Tenant permission evaluation over an already-resolved permission set.
//!
//! There is exactly one decision engine: the relation graph is resolved to a
//! permission set, and this evaluator checks that set (with the canonical
//! `manage` wildcard semantics from `permission_policy`). No external policy
//! engine or policy-file model is involved.

use crate::services::permission_policy::{check_all_permissions, check_any_permission, check_permission};
use rustok_api::Permission;

use super::permission_check::PermissionCheck;

pub async fn evaluate_policy_permissions(
    _tenant_id: &uuid::Uuid,
    resolved_permissions: &[Permission],
    permission_check: PermissionCheck<'_>,
) -> bool {
    match permission_check {
        PermissionCheck::Single(permission) => {
            check_permission(resolved_permissions, permission).allowed
        }
        PermissionCheck::Any(required) => {
            check_any_permission(resolved_permissions, required).allowed
        }
        PermissionCheck::All(required) => {
            check_all_permissions(resolved_permissions, required).allowed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::evaluate_policy_permissions;
    use crate::services::permission_check::PermissionCheck;
    use rustok_api::Permission;

    #[tokio::test]
    async fn policy_evaluator_allows_single_matching_permission() {
        let result = evaluate_policy_permissions(
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ],
            PermissionCheck::Single(&Permission::USERS_READ),
        )
        .await;
        assert!(result);
    }

    #[tokio::test]
    async fn policy_evaluator_denies_missing_permission() {
        let result = evaluate_policy_permissions(
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ],
            PermissionCheck::Single(&Permission::USERS_UPDATE),
        )
        .await;
        assert!(!result);
    }

    #[tokio::test]
    async fn policy_evaluator_any_all_respect_manage_wildcard() {
        let tenant_id = uuid::Uuid::new_v4();
        let permissions = [Permission::USERS_MANAGE];
        let allows_any = evaluate_policy_permissions(
            &tenant_id,
            &permissions,
            PermissionCheck::Any(&[Permission::USERS_READ, Permission::USERS_DELETE]),
        )
        .await;
        let allows_all = evaluate_policy_permissions(
            &tenant_id,
            &permissions,
            PermissionCheck::All(&[Permission::USERS_READ, Permission::USERS_UPDATE]),
        )
        .await;
        assert!(allows_any);
        assert!(allows_all);
    }
}
