use std::future::Future;

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use uuid::Uuid;

use super::auth::AuthContext;
use crate::permissions::{Permission, has_effective_permission};

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
/// The ordinary tenant permission snapshot does not create this context. Every
/// issued context is bound to a concrete operator actor for mutation audit
/// records; a nil or inferred tenant actor is not accepted by the constructor
/// contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAuthorityContext {
    authority: HostAuthority,
    actor_id: Uuid,
}

impl HostAuthorityContext {
    pub fn read(actor_id: Uuid) -> Option<Self> {
        Self::for_actor(HostAuthority::Read, actor_id)
    }

    pub fn manage(actor_id: Uuid) -> Option<Self> {
        Self::for_actor(HostAuthority::Manage, actor_id)
    }

    pub fn for_actor(authority: HostAuthority, actor_id: Uuid) -> Option<Self> {
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
            (HostAuthority::Read, HostAuthority::Read)
                | (HostAuthority::Manage, HostAuthority::Read)
                | (HostAuthority::Manage, HostAuthority::Manage)
        )
    }
}

tokio::task_local! {
    static HOST_AUTHORITY_SCOPE: Option<HostAuthorityContext>;
}

/// Run one request future under the already authenticated typed host authority.
///
/// The scope is the single owner of "which host authority is this request acting
/// with" for the whole process: the host credential middleware establishes it, and
/// every consumer — GraphQL resolvers, HTTP handlers and native server functions —
/// reads it through [`current_host_authority`]. It is deliberately request
/// task-local, so work that escapes the request task (WebSocket upgrades, detached
/// background tasks) does not inherit host-global authority and stays fail-closed.
pub async fn with_host_authority_scope<F>(
    authority: Option<HostAuthorityContext>,
    future: F,
) -> F::Output
where
    F: Future,
{
    HOST_AUTHORITY_SCOPE.scope(authority, future).await
}

/// Host authority carried by the current request, if any.
///
/// Returns `None` outside a request scope and for ordinary tenant requests, both of
/// which must be treated as "no host-global access".
pub fn current_host_authority() -> Option<HostAuthorityContext> {
    HOST_AUTHORITY_SCOPE
        .try_with(|authority| *authority)
        .ok()
        .flatten()
}

/// Authority rule for a host-owned administrative surface (diagnostics, platform
/// settings categories).
///
/// A caller is authorized when either
///
/// - the current request carries host-global authority that allows `host_authority`
///   (a host operator holding the host credential), or
/// - the authenticated permission snapshot contains `permission` (a tenant
///   administrator acting inside its own resolved tenant).
///
/// Both transports of the same surface must evaluate this one rule, so a browsable
/// admin page and an operator CLI cannot disagree about who may read or change the
/// same cache configuration. Tenant scoping is *not* covered here: callers that act
/// on tenant data must still reject a snapshot whose tenant differs from the
/// resolved tenant.
pub fn authorizes_host_or_permission(
    auth: Option<&AuthContext>,
    permission: &Permission,
    host_authority: HostAuthority,
) -> bool {
    if current_host_authority().is_some_and(|authority| authority.allows(host_authority)) {
        return true;
    }

    auth.is_some_and(|auth| has_effective_permission(&auth.permissions, permission))
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
    use super::{
        HostAuthority, HostAuthorityContext, authorizes_host_or_permission, current_host_authority,
        with_host_authority_scope,
    };
    use crate::context::AuthContext;
    use crate::permissions::Permission;
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
    fn generic_constructor_preserves_explicit_level() {
        let actor_id = Uuid::new_v4();
        let authority = HostAuthorityContext::for_actor(HostAuthority::Read, actor_id)
            .expect("non-nil operator actor");

        assert_eq!(authority.authority(), HostAuthority::Read);
        assert_eq!(authority.actor_id(), actor_id);
    }

    #[test]
    fn nil_operator_actor_is_rejected() {
        assert!(HostAuthorityContext::read(Uuid::nil()).is_none());
        assert!(HostAuthorityContext::manage(Uuid::nil()).is_none());
        assert!(HostAuthorityContext::for_actor(HostAuthority::Manage, Uuid::nil()).is_none());
    }

    fn admin_auth(permission: Permission) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![permission],
            client_id: None,
            scopes: Vec::new(),
            grant_type: "session".to_string(),
        }
    }

    #[test]
    fn admin_surface_rule_accepts_a_tenant_permission_without_host_authority() {
        let auth = admin_auth(Permission::SETTINGS_READ);
        assert!(authorizes_host_or_permission(
            Some(&auth),
            &Permission::SETTINGS_READ,
            HostAuthority::Read
        ));
        // `settings:read` is not enough for the managing level of the same surface.
        assert!(!authorizes_host_or_permission(
            Some(&auth),
            &Permission::SETTINGS_MANAGE,
            HostAuthority::Manage
        ));
        // `settings:manage` implies read through the effective-permission rule.
        let manager = admin_auth(Permission::SETTINGS_MANAGE);
        assert!(authorizes_host_or_permission(
            Some(&manager),
            &Permission::SETTINGS_READ,
            HostAuthority::Read
        ));
    }

    #[test]
    fn admin_surface_rule_rejects_a_caller_with_neither_authority() {
        assert!(!authorizes_host_or_permission(
            None,
            &Permission::SETTINGS_READ,
            HostAuthority::Read
        ));
        assert!(!authorizes_host_or_permission(
            Some(&admin_auth(Permission::USERS_READ)),
            &Permission::SETTINGS_READ,
            HostAuthority::Read
        ));
    }

    #[tokio::test]
    async fn host_scope_is_request_local_and_absent_outside_a_request() {
        assert!(current_host_authority().is_none());

        let operator = HostAuthorityContext::read(Uuid::new_v4()).expect("non-nil operator actor");
        let seen =
            with_host_authority_scope(Some(operator), async { current_host_authority() }).await;
        assert_eq!(seen, Some(operator));

        assert!(current_host_authority().is_none());

        let allowed = with_host_authority_scope(Some(operator), async {
            authorizes_host_or_permission(None, &Permission::SETTINGS_READ, HostAuthority::Read)
        })
        .await;
        assert!(
            allowed,
            "a host operator may read the surface without tenant RBAC"
        );

        let denied = with_host_authority_scope(Some(operator), async {
            authorizes_host_or_permission(None, &Permission::SETTINGS_MANAGE, HostAuthority::Manage)
        })
        .await;
        assert!(
            !denied,
            "a read-level host operator may not manage the surface"
        );
    }
}
