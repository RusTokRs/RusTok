use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use uuid::Uuid;

pub const HOST_AUTHORITY_REQUIRED: &str = "Host-global authority required";

/// Authority over process-wide operational state.
///
/// This is intentionally separate from tenant RBAC. Ordinary tenant roles,
/// broad tenant permissions, OAuth wildcards, and tenant identity never imply
/// host authority. Absence of this context means no host-global access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostAuthority {
    Read,
    Manage,
}

/// Request context issued only by a trusted host/operator authentication path.
///
/// The tenant authentication middleware does not create this context. Until an
/// explicit operator issuance path is composed, host-global transports remain
/// fail-closed. Every issued context is bound to a concrete operator actor for
/// mutation audit records; a nil or inferred tenant actor is not accepted by
/// the constructor contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAuthorityContext {
    authority: HostAuthority,
    actor_id: Uuid,
}

impl HostAuthorityContext {
    pub fn read(actor_id: Uuid) -> Option<Self> {
        Self::new(HostAuthority::Read, actor_id)
    }

    pub fn manage(actor_id: Uuid) -> Option<Self> {
        Self::new(HostAuthority::Manage, actor_id)
    }

    fn new(authority: HostAuthority, actor_id: Uuid) -> Option<Self> {
        (!actor_id.is_nil()).then_some(Self {
            authority,
            actor_id,
        })
    }

    pub const fn authority(self) -> HostAuthority {
        self.authority
    }

    pub const fn actor_id(self) -> Uuid {
        self.actor_id
    }

    pub const fn allows(self, required: HostAuthority) -> bool {
        matches!(
            (self.authority, required),
            (HostAuthority::Manage, _) | (HostAuthority::Read, HostAuthority::Read)
        )
    }
}

impl<S> FromRequestParts<S> for HostAuthorityContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<HostAuthorityContext>()
            .copied()
            .ok_or((StatusCode::FORBIDDEN, HOST_AUTHORITY_REQUIRED))
    }
}

#[cfg(test)]
mod tests {
    use super::{HostAuthority, HostAuthorityContext};
    use uuid::Uuid;

    #[test]
    fn read_authority_cannot_manage_host_state() {
        let actor_id = Uuid::new_v4();
        let authority = HostAuthorityContext::read(actor_id).expect("non-nil operator actor");

        assert_eq!(authority.actor_id(), actor_id);
        assert!(authority.allows(HostAuthority::Read));
        assert!(!authority.allows(HostAuthority::Manage));
    }

    #[test]
    fn manage_authority_includes_host_reads() {
        let authority =
            HostAuthorityContext::manage(Uuid::new_v4()).expect("non-nil operator actor");

        assert!(authority.allows(HostAuthority::Read));
        assert!(authority.allows(HostAuthority::Manage));
    }

    #[test]
    fn nil_operator_actor_is_rejected() {
        assert!(HostAuthorityContext::read(Uuid::nil()).is_none());
        assert!(HostAuthorityContext::manage(Uuid::nil()).is_none());
    }
}
