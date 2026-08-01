use async_graphql::{FieldError, Result};
use rustok_api::{AuthContext, AuthPrincipalKind, graphql::GraphQLError};
use uuid::Uuid;

pub(super) fn require_direct_control_plane_user(auth: &AuthContext, tenant_id: Uuid) -> Result<()> {
    let principal = crate::RbacControlPlanePrincipal {
        tenant_id: auth.tenant_id,
        principal_kind: auth.principal_kind,
    };
    crate::require_direct_control_plane_user(principal, tenant_id).map_err(|error| {
        let message = error.to_string();
        <FieldError as GraphQLError>::permission_denied(&message)
    })
}

#[cfg(test)]
mod tests {
    use super::require_direct_control_plane_user;
    use rustok_api::{AuthContext, AuthPrincipalKind};
    use uuid::Uuid;

    fn auth_context(tenant_id: Uuid, principal_kind: AuthPrincipalKind) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: if principal_kind.is_direct_user() {
                Uuid::new_v4()
            } else {
                Uuid::nil()
            },
            tenant_id,
            permissions: Vec::new(),
            principal_kind,
            client_id: if principal_kind.is_direct_user() {
                None
            } else {
                Some(Uuid::new_v4())
            },
            scopes: Vec::new(),
            grant_type: match principal_kind {
                AuthPrincipalKind::DirectUser => "direct",
                AuthPrincipalKind::DelegatedUser => "authorization_code",
                AuthPrincipalKind::Service => "client_credentials",
            }
            .to_string(),
        }
    }

    #[test]
    fn direct_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id, AuthPrincipalKind::DirectUser);

        assert!(require_direct_control_plane_user(&auth, tenant_id).is_ok());
    }

    #[test]
    fn delegated_and_service_principals_are_denied() {
        for principal_kind in [
            AuthPrincipalKind::DelegatedUser,
            AuthPrincipalKind::Service,
        ] {
            let auth = auth_context(Uuid::new_v4(), principal_kind);

            assert!(require_direct_control_plane_user(&auth, auth.tenant_id).is_err());
        }
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let auth = auth_context(Uuid::new_v4(), AuthPrincipalKind::DirectUser);

        assert!(require_direct_control_plane_user(&auth, Uuid::new_v4()).is_err());
    }
}
