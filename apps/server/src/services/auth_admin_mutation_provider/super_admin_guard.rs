use rustok_auth::AuthAdminMutationError;
use rustok_core::{UserRole, UserStatus};
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

pub(super) async fn ensure_active_super_admin_continuity(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    target_user_id: Uuid,
    resulting_role: Option<&UserRole>,
    resulting_status: Option<&UserStatus>,
    deleting: bool,
) -> Result<(), AuthAdminMutationError> {
    rustok_rbac::ensure_user_authority_continuity_on(
        db,
        tenant_id,
        target_user_id,
        &rustok_rbac::RbacUserAuthorityChange {
            role: resulting_role.cloned(),
            status: resulting_status.cloned(),
            deleting,
        },
    )
    .await
    .map_err(persistence_error)
}

pub(super) fn persistence_error(
    error: rustok_rbac::RbacRolePersistenceError,
) -> AuthAdminMutationError {
    match error {
        rustok_rbac::RbacRolePersistenceError::TargetNotFound => {
            AuthAdminMutationError::NotFound("user".to_string())
        }
        rustok_rbac::RbacRolePersistenceError::Policy(policy) => {
            super::user_admin::map_role_mutation_policy_error(policy)
        }
        rustok_rbac::RbacRolePersistenceError::InconsistentAuthority => {
            AuthAdminMutationError::Conflict(
                "active super administrator role assignment is inconsistent".to_string(),
            )
        }
        other => AuthAdminMutationError::Internal(other.to_string()),
    }
}
