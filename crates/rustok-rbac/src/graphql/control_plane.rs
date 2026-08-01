use async_graphql::{FieldError, Result};
use rustok_api::{AuthContext, graphql::GraphQLError};
use uuid::Uuid;

pub(super) fn require_direct_control_plane_user(auth: &AuthContext, tenant_id: Uuid) -> Result<()> {
    let kind = auth.validated_principal_kind().map_err(|error| {
        let message = error.to_string();
        <FieldError as GraphQLError>::permission_denied(&message)
    })?;
    let principal = crate::RbacControlPlanePrincipal {
        tenant_id: auth.tenant_id,
        session_id: auth.session_id,
        kind,
    };
    crate::require_direct_control_plane_user(principal, tenant_id).map_err(|error| {
        let message = error.to_string();
        <FieldError as GraphQLError>::permission_denied(&message)
    })
}

#[cfg(test)]
mod tests {
    use super::require_direct_control_plane_user;
    use rustok_api::AuthContext;
    use uuid::Uuid;

    fn auth_context(
        tenant_id: Uuid,
        session_id: Uuid,
        client_id: Option<Uuid>,
        grant_type: &str,
    ) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id,
            tenant_id,
            permissions: Vec::new(),
            client_id,
            scopes: Vec::new(),
            grant_type: grant_type.to_string(),
        }
    }

    #[test]
    fn direct_session_bound_user_is_allowed_for_matching_tenant() {
        let tenant_id = Uuid::new_v4();
        let auth = auth_context(tenant_id, Uuid::new_v4(), None, "direct");

        assert!(require_direct_control_plane_user(&auth, tenant_id).is_ok());
    }

    #[test]
    fn delegated_and_service_principals_are_denied() {
        for grant_type in ["authorization_code", "client_credentials"] {
            let auth = auth_context(
                Uuid::new_v4(),
                Uuid::nil(),
                Some(Uuid::new_v4()),
                grant_type,
            );

            assert!(require_direct_control_plane_user(&auth, auth.tenant_id).is_err());
        }
    }

    #[test]
    fn malformed_authenticated_facts_are_denied_before_owner_admission() {
        for auth in [
            auth_context(Uuid::new_v4(), Uuid::nil(), None, "direct"),
            auth_context(
                Uuid::new_v4(),
                Uuid::new_v4(),
                Some(Uuid::new_v4()),
                "authorization_code",
            ),
            auth_context(Uuid::new_v4(), Uuid::new_v4(), None, "password"),
        ] {
            assert!(require_direct_control_plane_user(&auth, auth.tenant_id).is_err());
        }
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let auth = auth_context(Uuid::new_v4(), Uuid::new_v4(), None, "direct");

        assert!(require_direct_control_plane_user(&auth, Uuid::new_v4()).is_err());
    }
}
