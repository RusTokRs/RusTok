use async_graphql::{FieldError, Result};
use rustok_api::{AuthContext, graphql::GraphQLError};
use uuid::Uuid;

pub(super) fn require_direct_control_plane_user(auth: &AuthContext, tenant_id: Uuid) -> Result<()> {
    if auth.client_id.is_some() || auth.grant_type != "direct" || auth.session_id.is_nil() {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "RBAC control plane requires a direct, session-bound user principal",
        ));
    }

    if auth.tenant_id != tenant_id {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "authenticated principal belongs to another tenant",
        ));
    }

    Ok(())
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
    fn oauth_principals_are_denied() {
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
    fn malformed_direct_principal_without_session_is_denied() {
        let auth = auth_context(Uuid::new_v4(), Uuid::nil(), None, "direct");

        assert!(require_direct_control_plane_user(&auth, auth.tenant_id).is_err());
    }

    #[test]
    fn cross_tenant_context_is_denied() {
        let auth = auth_context(Uuid::new_v4(), Uuid::new_v4(), None, "direct");

        assert!(require_direct_control_plane_user(&auth, Uuid::new_v4()).is_err());
    }
}
