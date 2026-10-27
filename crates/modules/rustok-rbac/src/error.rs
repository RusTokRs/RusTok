use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RbacError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("role not found: {0}")]
    RoleNotFound(String),
    #[error("user not found: {0}")]
    UserNotFound(uuid::Uuid),
    #[error("tenant mismatch")]
    TenantMismatch,
    #[error("validation error: {0}")]
    Validation(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<crate::RbacRoleMutationPolicyError> for RbacError {
    fn from(err: crate::RbacRoleMutationPolicyError) -> Self {
        match err {
            crate::RbacRoleMutationPolicyError::ActorTenantMismatch
            | crate::RbacRoleMutationPolicyError::TargetTenantMismatch => Self::TenantMismatch,
            other => Self::Validation(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_error_display_and_conversion() {
        assert_eq!(RbacError::PermissionDenied.to_string(), "permission denied");
        assert_eq!(
            RbacError::RoleNotFound("manager".to_string()).to_string(),
            "role not found: manager"
        );
        let user_id = uuid::Uuid::nil();
        assert_eq!(
            RbacError::UserNotFound(user_id).to_string(),
            format!("user not found: {user_id}")
        );
        assert_eq!(RbacError::TenantMismatch.to_string(), "tenant mismatch");
        assert_eq!(
            RbacError::Validation("invalid name".to_string()).to_string(),
            "validation error: invalid name"
        );
        assert_eq!(
            RbacError::Internal("db fail".to_string()).to_string(),
            "internal error: db fail"
        );

        let policy_err = crate::RbacRoleMutationPolicyError::ActorTenantMismatch;
        let rbac_err: RbacError = policy_err.into();
        assert_eq!(rbac_err, RbacError::TenantMismatch);

        let policy_err_nil = crate::RbacRoleMutationPolicyError::NilIdentity("tenant_id");
        let rbac_err_nil: RbacError = policy_err_nil.into();
        assert!(matches!(rbac_err_nil, RbacError::Validation(_)));
    }
}
