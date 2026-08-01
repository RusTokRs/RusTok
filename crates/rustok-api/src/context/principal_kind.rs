use uuid::Uuid;

/// Trusted principal classification produced after access-token subject and grant
/// invariants have been validated by the authentication boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthPrincipalKind {
    DirectUser,
    DelegatedUser,
    Service,
}

impl AuthPrincipalKind {
    /// Convert already-authenticated grant facts into one explicit principal kind.
    /// Invalid or ambiguous combinations fail closed and must not reach downstream
    /// authorization policy.
    pub fn from_authenticated_facts(
        grant_type: &str,
        client_id: Option<Uuid>,
        session_id: Uuid,
    ) -> Option<Self> {
        match grant_type {
            "direct" if client_id.is_none() && !session_id.is_nil() => Some(Self::DirectUser),
            "authorization_code" if client_id.is_some() && session_id.is_nil() => {
                Some(Self::DelegatedUser)
            }
            "client_credentials" if client_id.is_some() && session_id.is_nil() => {
                Some(Self::Service)
            }
            _ => None,
        }
    }

    pub const fn is_direct_user(self) -> bool {
        matches!(self, Self::DirectUser)
    }

    pub const fn is_human_user(self) -> bool {
        matches!(self, Self::DirectUser | Self::DelegatedUser)
    }

    pub const fn is_service(self) -> bool {
        matches!(self, Self::Service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_facts_classify_fail_closed() {
        assert_eq!(
            AuthPrincipalKind::from_authenticated_facts("direct", None, Uuid::new_v4()),
            Some(AuthPrincipalKind::DirectUser)
        );
        assert_eq!(
            AuthPrincipalKind::from_authenticated_facts(
                "authorization_code",
                Some(Uuid::new_v4()),
                Uuid::nil(),
            ),
            Some(AuthPrincipalKind::DelegatedUser)
        );
        assert_eq!(
            AuthPrincipalKind::from_authenticated_facts(
                "client_credentials",
                Some(Uuid::new_v4()),
                Uuid::nil(),
            ),
            Some(AuthPrincipalKind::Service)
        );
        assert_eq!(
            AuthPrincipalKind::from_authenticated_facts(
                "authorization_code",
                Some(Uuid::new_v4()),
                Uuid::new_v4(),
            ),
            None
        );
        assert_eq!(
            AuthPrincipalKind::from_authenticated_facts("direct", Some(Uuid::new_v4()), Uuid::new_v4()),
            None
        );
    }
}
