use async_graphql::{FieldError, Result};
use rustok_api::{AuthContext, AuthPrincipalContext, graphql::GraphQLError};
use uuid::Uuid;

pub(super) fn require_direct_control_plane_user(
    auth: &AuthContext,
    principal_context: AuthPrincipalContext,
    tenant_id: Uuid,
) -> Result<()> {
    let principal = crate::RbacControlPlanePrincipal {
        tenant_id: auth.tenant_id,
        principal_kind: principal_context.kind,
    };
    crate::require_direct_control_plane_user(principal, tenant_id).map_err(|error| {
        let message = error.to_string();
        <FieldError as GraphQLError>::permission_denied(&message)
    })
}

#[cfg(test)]
mod tests {
    use super::require_direct_control_plane_user;
    use rustok_api::{AuthContext, AuthPrincipalContext, AuthPrincipalKind};
    use uuid::Uuid;

    fn auth_context(tenant_id: Uuid) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: Vec::new(),
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }
    }

    #[test]
    fn direct_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id);

        assert!(
            require_direct_control_plane_user(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                tenant_id,
            )
            .is_ok()
        );
    }

    #[test]
    fn delegated_and_service_principals_are_denied() {
        for principal_kind in [AuthPrincipalKind::DelegatedUser, AuthPrincipalKind::Service] {
            let auth = auth_context(Uuid::new_v4());

            assert!(
                require_direct_control_plane_user(
                    &auth,
                    AuthPrincipalContext::new(principal_kind),
                    auth.tenant_id,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let auth = auth_context(Uuid::new_v4());

        assert!(
            require_direct_control_plane_user(
                &auth,
                AuthPrincipalContext::new(AuthPrincipalKind::DirectUser),
                Uuid::new_v4(),
            )
            .is_err()
        );
    }
}
