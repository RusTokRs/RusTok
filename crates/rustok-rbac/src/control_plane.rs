use rustok_api::AuthPrincipalKind;
use thiserror::Error;
use uuid::Uuid;

/// Host-neutral authenticated facts required by the RBAC control-plane policy.
///
/// The host supplies one already validated principal kind from the shared
/// authorization context. RBAC does not reinterpret grant strings, client ids,
/// or OAuth subject shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbacControlPlanePrincipal {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub kind: AuthPrincipalKind,
}

/// Fail-closed admission errors for tenant-scoped RBAC control-plane operations.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RbacControlPlaneAdmissionError {
    #[error("RBAC control plane requires a direct, session-bound user principal")]
    DirectSessionRequired,
    #[error("authenticated principal belongs to another tenant")]
    TenantMismatch,
}

/// Require a direct, session-bound user whose authenticated tenant matches the
/// routed tenant before any RBAC role or permission control-plane admission.
///
/// OAuth delegated users and service principals remain valid for data-plane
/// operations admitted by their effective permissions, but they are not allowed
/// to mutate or inspect RBAC control-plane state.
pub fn require_direct_control_plane_user(
    principal: RbacControlPlanePrincipal,
    tenant_id: Uuid,
) -> Result<(), RbacControlPlaneAdmissionError> {
    if principal.kind != AuthPrincipalKind::DirectUser || principal.session_id.is_nil() {
        return Err(RbacControlPlaneAdmissionError::DirectSessionRequired);
    }

    if principal.tenant_id != tenant_id {
        return Err(RbacControlPlaneAdmissionError::TenantMismatch);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(
        tenant_id: Uuid,
        session_id: Uuid,
        kind: AuthPrincipalKind,
    ) -> RbacControlPlanePrincipal {
        RbacControlPlanePrincipal {
            tenant_id,
            session_id,
            kind,
        }
    }

    #[test]
    fn direct_session_bound_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let principal = principal(
            tenant_id,
            Uuid::new_v4(),
            AuthPrincipalKind::DirectUser,
        );

        assert_eq!(
            require_direct_control_plane_user(principal, tenant_id),
            Ok(())
        );
    }

    #[test]
    fn delegated_and_service_principals_are_denied_even_with_a_session_value() {
        for kind in [
            AuthPrincipalKind::DelegatedUser,
            AuthPrincipalKind::Service,
        ] {
            let tenant_id = Uuid::new_v4();
            let principal = principal(tenant_id, Uuid::new_v4(), kind);

            assert_eq!(
                require_direct_control_plane_user(principal, tenant_id),
                Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
            );
        }
    }

    #[test]
    fn direct_principal_without_session_is_denied() {
        let tenant_id = Uuid::new_v4();
        let principal = principal(tenant_id, Uuid::nil(), AuthPrincipalKind::DirectUser);

        assert_eq!(
            require_direct_control_plane_user(principal, tenant_id),
            Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
        );
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let principal = principal(
            Uuid::new_v4(),
            Uuid::new_v4(),
            AuthPrincipalKind::DirectUser,
        );

        assert_eq!(
            require_direct_control_plane_user(principal, Uuid::new_v4()),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );
    }
}
