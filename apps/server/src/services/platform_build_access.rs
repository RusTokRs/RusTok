use rustok_api::{HostAuthority, HostAuthorityContext};

/// Whether the authenticated request carries explicit host-level read authority.
/// Platform build state is global, so tenant roles and permissions must not grant access.
pub fn is_platform_build_operator(authority: Option<HostAuthorityContext>) -> bool {
    authority.is_some_and(|authority| authority.allows(HostAuthority::Read))
}

#[cfg(test)]
mod tests {
    use super::is_platform_build_operator;
    use rustok_api::HostAuthorityContext;
    use uuid::Uuid;

    #[test]
    fn platform_build_access_requires_explicit_host_authority() {
        assert!(!is_platform_build_operator(None));

        // Tenant-level permissions are intentionally not an input to this
        // host-global admission check.
        assert!(!is_platform_build_operator(None));

        let reader = HostAuthorityContext::read(Uuid::new_v4()).expect("valid host actor");
        assert!(is_platform_build_operator(Some(reader)));

        let manager = HostAuthorityContext::manage(Uuid::new_v4()).expect("valid host actor");
        assert!(is_platform_build_operator(Some(manager)));
    }

    #[test]
    fn nil_host_actor_cannot_create_build_authority() {
        assert!(HostAuthorityContext::read(Uuid::nil()).is_none());
        assert!(HostAuthorityContext::manage(Uuid::nil()).is_none());
    }
}
