use super::{auth::AuthContext, principal_kind::AuthPrincipalKind};
use crate::PortActor;
use std::fmt;
use uuid::Uuid;

const DIRECT_GRANT: &str = "direct";
const AUTHORIZATION_CODE_GRANT: &str = "authorization_code";
const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";

/// Fail-closed error returned when authenticated principal facts do not match
/// one supported direct-user, delegated-user, or service shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthPrincipalKindError {
    InvalidAuthenticatedFacts,
}

impl fmt::Display for AuthPrincipalKindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAuthenticatedFacts => formatter.write_str(
                "authenticated principal facts do not match a supported principal kind",
            ),
        }
    }
}

impl std::error::Error for AuthPrincipalKindError {}

impl AuthContext {
    /// Resolve one typed principal kind from the authenticated context.
    ///
    /// This is the single migration classifier for legacy grant/session/client
    /// facts. Authorization owners must consume the returned enum and must not
    /// reinterpret those facts independently.
    pub fn validated_principal_kind(
        &self,
    ) -> Result<AuthPrincipalKind, AuthPrincipalKindError> {
        match self.grant_type.as_str() {
            DIRECT_GRANT if self.client_id.is_none() && !self.session_id.is_nil() => {
                Ok(AuthPrincipalKind::DirectUser)
            }
            AUTHORIZATION_CODE_GRANT
                if self.client_id.is_some() && self.session_id.is_nil() =>
            {
                Ok(AuthPrincipalKind::DelegatedUser)
            }
            CLIENT_CREDENTIALS_GRANT
                if self.client_id.is_some() && self.session_id.is_nil() =>
            {
                Ok(AuthPrincipalKind::Service)
            }
            _ => Err(AuthPrincipalKindError::InvalidAuthenticatedFacts),
        }
    }

    /// Whether this request represents an OAuth service principal rather than a
    /// human user. Invalid authenticated facts fail closed and are not treated
    /// as either a service or human principal.
    pub fn is_service_principal(&self) -> bool {
        matches!(
            self.validated_principal_kind(),
            Ok(AuthPrincipalKind::Service)
        )
    }

    pub fn is_human_user_principal(&self) -> bool {
        matches!(
            self.validated_principal_kind(),
            Ok(AuthPrincipalKind::DirectUser | AuthPrincipalKind::DelegatedUser)
        )
    }

    /// Return a user id only for validated human-user grants. Use this for
    /// legacy `created_by: Option<Uuid>` columns that cannot represent actor kind.
    pub fn human_user_id(&self) -> Option<Uuid> {
        self.is_human_user_principal().then_some(self.user_id)
    }

    /// Preserve principal kind when crossing a transport-agnostic service port.
    ///
    /// Existing port consumers still receive the established user/service
    /// contract. Invalid legacy facts remain fail-closed for human-user helpers
    /// and must be rejected by authorization admission before this mapping is
    /// used for a state-changing operation.
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

    fn auth(
        grant_type: &str,
        session_id: Uuid,
        client_id: Option<Uuid>,
    ) -> AuthContext {
        AuthContext {
            user_id: client_id.unwrap_or_else(Uuid::new_v4),
            session_id,
            tenant_id: Uuid::new_v4(),
            permissions: vec![Permission::PRODUCTS_READ],
            client_id,
            scopes: Vec::new(),
            grant_type: grant_type.to_string(),
        }
    }

    #[test]
    fn direct_user_kind_requires_a_session_and_no_client() {
        let auth = auth(DIRECT_GRANT, Uuid::new_v4(), None);
        assert_eq!(
            auth.validated_principal_kind(),
            Ok(AuthPrincipalKind::DirectUser)
        );
        assert!(auth.is_human_user_principal());
        assert_eq!(auth.human_user_id(), Some(auth.user_id));
    }

    #[test]
    fn authorization_code_kind_is_delegated_user() {
        let auth = auth(
            AUTHORIZATION_CODE_GRANT,
            Uuid::nil(),
            Some(Uuid::new_v4()),
        );
        assert_eq!(
            auth.validated_principal_kind(),
            Ok(AuthPrincipalKind::DelegatedUser)
        );
        assert!(auth.is_human_user_principal());
        assert_eq!(auth.human_user_id(), Some(auth.user_id));
    }

    #[test]
    fn client_credentials_map_to_service_port_actor_without_user_id() {
        let client_id = Uuid::new_v4();
        let auth = auth(
            CLIENT_CREDENTIALS_GRANT,
            Uuid::nil(),
            Some(client_id),
        );
        assert_eq!(
            auth.validated_principal_kind(),
            Ok(AuthPrincipalKind::Service)
        );
        assert!(auth.is_service_principal());
        assert_eq!(auth.human_user_id(), None);
        assert_eq!(auth.port_actor().kind, PortActorKind::Service);
        assert_eq!(auth.port_actor().id, client_id.to_string());
    }

    #[test]
    fn malformed_authenticated_facts_fail_closed() {
        for auth in [
            auth(DIRECT_GRANT, Uuid::nil(), None),
            auth(DIRECT_GRANT, Uuid::new_v4(), Some(Uuid::new_v4())),
            auth(AUTHORIZATION_CODE_GRANT, Uuid::new_v4(), Some(Uuid::new_v4())),
            auth(AUTHORIZATION_CODE_GRANT, Uuid::nil(), None),
            auth(CLIENT_CREDENTIALS_GRANT, Uuid::new_v4(), Some(Uuid::new_v4())),
            auth(CLIENT_CREDENTIALS_GRANT, Uuid::nil(), None),
            auth("password", Uuid::new_v4(), None),
        ] {
            assert_eq!(
                auth.validated_principal_kind(),
                Err(AuthPrincipalKindError::InvalidAuthenticatedFacts)
            );
            assert!(!auth.is_service_principal());
            assert!(!auth.is_human_user_principal());
            assert_eq!(auth.human_user_id(), None);
        }
    }
}
