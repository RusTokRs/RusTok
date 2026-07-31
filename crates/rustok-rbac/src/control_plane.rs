use thiserror::Error;
use uuid::Uuid;

/// Host-neutral authenticated facts required by the RBAC control-plane policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbacControlPlanePrincipal<'a> {
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub client_id: Option<Uuid>,
    pub grant_type: &'a str,
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
/// OAuth authorization-code and client-credentials principals remain valid for
/// data-plane operations admitted by their effective permissions, but they are
/// not allowed to mutate RBAC control-plane state.
pub fn require_direct_control_plane_user(
    principal: RbacControlPlanePrincipal<'_>,
    tenant_id: Uuid,
) -> Result<(), RbacControlPlaneAdmissionError> {
    if principal.client_id.is_some()
        || principal.grant_type != "direct"
        || principal.session_id.is_nil()
    {
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
        client_id: Option<Uuid>,
        grant_type: &str,
    ) -> RbacControlPlanePrincipal<'_> {
        RbacControlPlanePrincipal {
            tenant_id,
            session_id,
            client_id,
            grant_type,
        }
    }

    #[test]
    fn direct_session_bound_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let principal = principal(tenant_id, Uuid::new_v4(), None, "direct");

        assert_eq!(
            require_direct_control_plane_user(principal, tenant_id),
            Ok(())
        );
    }

    #[test]
    fn oauth_principals_are_denied_even_with_management_permission() {
        for grant_type in ["authorization_code", "client_credentials"] {
            let tenant_id = Uuid::new_v4();
            let principal = principal(
                tenant_id,
                Uuid::nil(),
                Some(Uuid::new_v4()),
                grant_type,
            );

            assert_eq!(
                require_direct_control_plane_user(principal, tenant_id),
                Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
            );
        }
    }

    #[test]
    fn malformed_direct_principal_without_session_is_denied() {
        let tenant_id = Uuid::new_v4();
        let principal = principal(tenant_id, Uuid::nil(), None, "direct");

        assert_eq!(
            require_direct_control_plane_user(principal, tenant_id),
            Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
        );
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let principal = principal(Uuid::new_v4(), Uuid::new_v4(), None, "direct");

        assert_eq!(
            require_direct_control_plane_user(principal, Uuid::new_v4()),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );
    }
}
