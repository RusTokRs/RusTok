use super::auth::AuthContext;
use crate::PortActor;

const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";

impl AuthContext {
    /// Whether this request represents an OAuth service principal rather than a
    /// human user. Authentication has already validated the grant/subject
    /// invariants before constructing this context.
    pub fn is_service_principal(&self) -> bool {
        self.grant_type == CLIENT_CREDENTIALS_GRANT
    }

    pub fn is_human_user_principal(&self) -> bool {
        !self.is_service_principal()
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
    use uuid::Uuid;

    fn auth(grant_type: &str, client_id: Option<Uuid>) -> AuthContext {
        AuthContext {
            user_id: client_id.unwrap_or_else(Uuid::new_v4),
            session_id: if grant_type == CLIENT_CREDENTIALS_GRANT {
                Uuid::nil()
            } else {
                Uuid::new_v4()
            },
            tenant_id: Uuid::new_v4(),
            permissions: vec![Permission::PRODUCTS_READ],
            client_id,
            scopes: Vec::new(),
            grant_type: grant_type.to_string(),
        }
    }

    #[test]
    fn client_credentials_map_to_service_port_actor() {
        let client_id = Uuid::new_v4();
        let auth = auth(CLIENT_CREDENTIALS_GRANT, Some(client_id));
        assert!(auth.is_service_principal());
        assert_eq!(auth.port_actor().kind, PortActorKind::Service);
        assert_eq!(auth.port_actor().id, client_id.to_string());
    }

    #[test]
    fn direct_and_authorization_code_grants_map_to_user_actor() {
        for grant in ["direct", "authorization_code"] {
            let auth = auth(grant, None);
            assert!(auth.is_human_user_principal());
            assert_eq!(auth.port_actor().kind, PortActorKind::User);
            assert_eq!(auth.port_actor().id, auth.user_id.to_string());
        }
    }
}
