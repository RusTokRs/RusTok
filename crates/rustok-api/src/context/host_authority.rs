use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

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
/// fail-closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAuthorityContext {
    authority: HostAuthority,
}

impl HostAuthorityContext {
    pub const fn read() -> Self {
        Self {
            authority: HostAuthority::Read,
        }
    }

    pub const fn manage() -> Self {
        Self {
            authority: HostAuthority::Manage,
        }
    }

    pub const fn authority(self) -> HostAuthority {
        self.authority
    }

    pub const fn allows(self, required: HostAuthority) -> bool {
        matches!(
            (self.authority, required),
            (HostAuthority::Manage, _)
                | (HostAuthority::Read, HostAuthority::Read)
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

    #[test]
    fn read_authority_cannot_manage_host_state() {
        let authority = HostAuthorityContext::read();

        assert!(authority.allows(HostAuthority::Read));
        assert!(!authority.allows(HostAuthority::Manage));
    }

    #[test]
    fn manage_authority_includes_host_reads() {
        let authority = HostAuthorityContext::manage();

        assert!(authority.allows(HostAuthority::Read));
        assert!(authority.allows(HostAuthority::Manage));
    }
}
