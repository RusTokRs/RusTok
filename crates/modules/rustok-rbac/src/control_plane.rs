use rustok_api::AuthPrincipalKind;
use thiserror::Error;
use uuid::Uuid;

/// Host-neutral authenticated facts required by the RBAC control-plane policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RbacControlPlanePrincipal {
    pub tenant_id: Uuid,
    pub principal_kind: AuthPrincipalKind,
}

/// Fail-closed admission errors for tenant-scoped RBAC control-plane operations.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum RbacControlPlaneAdmissionError {
    #[error("RBAC control plane requires a direct, session-bound user principal")]
    DirectSessionRequired,
    #[error("authenticated principal belongs to another tenant")]
    TenantMismatch,
}

/// Require a direct user whose authenticated tenant matches the routed tenant
/// before any RBAC role or permission control-plane admission.
///
/// Authentication validates grant, client and session invariants once and stores
/// the resulting typed principal kind in the shared authorization context.
/// Delegated users and services remain valid for data-plane operations admitted
/// by their effective permissions, but they cannot enter RBAC control-plane state.
pub fn require_direct_control_plane_user(
    principal: RbacControlPlanePrincipal,
    tenant_id: Uuid,
) -> Result<(), RbacControlPlaneAdmissionError> {
    if !principal.principal_kind.is_direct_user() {
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

    fn principal(tenant_id: Uuid, principal_kind: AuthPrincipalKind) -> RbacControlPlanePrincipal {
        RbacControlPlanePrincipal {
            tenant_id,
            principal_kind,
        }
    }

    #[test]
    fn direct_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();

        assert_eq!(
            require_direct_control_plane_user(
                principal(tenant_id, AuthPrincipalKind::DirectUser),
                tenant_id,
            ),
            Ok(())
        );
    }

    #[test]
    fn delegated_and_service_principals_are_denied_even_with_management_permission() {
        for principal_kind in [AuthPrincipalKind::DelegatedUser, AuthPrincipalKind::Service] {
            let tenant_id = Uuid::new_v4();

            assert_eq!(
                require_direct_control_plane_user(principal(tenant_id, principal_kind), tenant_id,),
                Err(RbacControlPlaneAdmissionError::DirectSessionRequired)
            );
        }
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let principal = principal(Uuid::new_v4(), AuthPrincipalKind::DirectUser);

        assert_eq!(
            require_direct_control_plane_user(principal, Uuid::new_v4()),
            Err(RbacControlPlaneAdmissionError::TenantMismatch)
        );
    }
}
