use super::auth::{AuthContext, AuthPrincipalKind};
use crate::PortActor;
use uuid::Uuid;

impl AuthContext {
    /// Whether this request represents an OAuth service principal rather than a
    /// human user. Authentication classifies this once before constructing the
    /// shared context.
    pub fn is_service_principal(&self) -> bool {
        self.principal_kind.is_service()
    }

    pub fn is_human_user_principal(&self) -> bool {
        self.principal_kind.is_human_user()
    }

    pub fn is_direct_user_principal(&self) -> bool {
        self.principal_kind.is_direct_user()
    }

    /// Return a user id only for human-user grants. Use this for legacy
    /// `created_by: Option<Uuid>` columns that cannot represent actor kind.
    pub fn human_user_id(&self) -> Option<Uuid> {
        self.is_human_user_principal().then_some(self.user_id)
    }

    /// Preserve principal kind when crossing a transport-agnostic service port.
    pub fn port_actor(&self) -> PortActor {
        if self.is_service_principal() {
            PortActor::service(self.client_id.unwrap_or(self.user_id).to_string())
        } else {
            PortActor::user(self.user_id.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Permission, PortActorKind};

    fn auth(principal_kind: AuthPrincipalKind, client_id: Option<Uuid>) -> AuthContext {
        AuthContext {
            user_id: client_id.unwrap_or_else(Uuid::new_v4),
            session_id: if principal_kind.is_direct_user() {
                Uuid::new_v4()
            } else {
                Uuid::nil()
            },
            tenant_id: Uuid::new_v4(),
            permissions: vec![Permission::PRODUCTS_READ],
            principal_kind,
            client_id,
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
    fn service_kind_maps_to_service_port_actor_without_user_id() {
        let client_id = Uuid::new_v4();
        let auth = auth(AuthPrincipalKind::Service, Some(client_id));
        assert!(auth.is_service_principal());
        assert_eq!(auth.human_user_id(), None);
        assert_eq!(auth.port_actor().kind, PortActorKind::Service);
        assert_eq!(auth.port_actor().id, client_id.to_string());
    }

    #[test]
    fn direct_and_delegated_users_map_to_user_actor() {
        for principal_kind in [
            AuthPrincipalKind::DirectUser,
            AuthPrincipalKind::DelegatedUser,
        ] {
            let auth = auth(principal_kind, None);
            assert!(auth.is_human_user_principal());
            assert_eq!(auth.is_direct_user_principal(), principal_kind.is_direct_user());
            assert_eq!(auth.human_user_id(), Some(auth.user_id));
            assert_eq!(auth.port_actor().kind, PortActorKind::User);
            assert_eq!(auth.port_actor().id, auth.user_id.to_string());
        }
    }
}
