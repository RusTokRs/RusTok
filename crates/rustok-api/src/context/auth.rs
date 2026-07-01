use crate::{Action, Permission};
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use uuid::Uuid;

/// Check if a requested scope is allowed by the granted scope list.
///
/// Supports:
/// - exact matches like `catalog:read`
/// - resource wildcards like `catalog:*`
/// - global wildcard `*:*`
pub fn scope_matches(allowed: &[String], requested: &str) -> bool {
    for allowed_scope in allowed {
        if allowed_scope == "*:*" {
            return true;
        }
        if allowed_scope == requested {
            return true;
        }
        if let Some(prefix) = allowed_scope.strip_suffix(":*") {
            if let Some(req_prefix) = requested.split(':').next() {
                if prefix == req_prefix {
                    return true;
                }
            }
        }
    }

    false
}

pub fn has_effective_permission(permissions: &[Permission], required: &Permission) -> bool {
    permissions.contains(required)
        || permissions.contains(&Permission::new(required.resource, Action::Manage))
}

pub fn has_any_effective_permission(permissions: &[Permission], required: &[Permission]) -> bool {
    required
        .iter()
        .any(|permission| has_effective_permission(permissions, permission))
}

#[derive(Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub permissions: Vec<Permission>,
    pub client_id: Option<Uuid>,
    pub scopes: Vec<String>,
    pub grant_type: String,
}

#[derive(Clone)]
pub struct AuthContextExtension(pub AuthContext);

impl AuthContext {
    /// Check if the current context has the required scope.
    /// For direct grants (embedded/user login), scopes are empty and access is allowed.
    /// For OAuth2 tokens, scopes must include the required scope (with wildcard support).
    pub fn require_scope(&self, required: &str) -> Result<(), async_graphql::Error> {
        if self.client_id.is_none() {
            return Ok(());
        }

        if scope_matches(&self.scopes, required) {
            return Ok(());
        }

        Err(async_graphql::Error::new(format!(
            "Insufficient scope: required '{}', granted: {:?}",
            required, self.scopes
        )))
    }
}

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContextExtension>()
            .map(|ext| ext.0.clone())
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Authentication required".to_string(),
            ))
    }
}

pub struct OptionalAuthContext(pub Option<AuthContext>);

impl<S> FromRequestParts<S> for OptionalAuthContext
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<AuthContextExtension>()
                .map(|ext| ext.0.clone()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_auth_ctx(client_id: Option<Uuid>, scopes: Vec<String>) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            permissions: vec![],
            client_id,
            scopes,
            grant_type: if client_id.is_some() {
                "client_credentials".to_string()
            } else {
                "direct".to_string()
            },
        }
    }

    #[test]
    fn require_scope_direct_grant_always_allowed() {
        let ctx = make_auth_ctx(None, vec![]);
        assert!(ctx.require_scope("catalog:read").is_ok());
        assert!(ctx.require_scope("admin:users").is_ok());
        assert!(ctx.require_scope("anything").is_ok());
    }

    #[test]
    fn require_scope_oauth_exact_match() {
        let ctx = make_auth_ctx(
            Some(Uuid::new_v4()),
            vec!["catalog:read".to_string(), "orders:write".to_string()],
        );
        assert!(ctx.require_scope("catalog:read").is_ok());
        assert!(ctx.require_scope("orders:write").is_ok());
        assert!(ctx.require_scope("admin:users").is_err());
    }

    #[test]
    fn require_scope_oauth_wildcard() {
        let ctx = make_auth_ctx(Some(Uuid::new_v4()), vec!["storefront:*".to_string()]);
        assert!(ctx.require_scope("storefront:read").is_ok());
        assert!(ctx.require_scope("storefront:write").is_ok());
        assert!(ctx.require_scope("admin:read").is_err());
    }

    #[test]
    fn require_scope_oauth_superadmin() {
        let ctx = make_auth_ctx(Some(Uuid::new_v4()), vec!["*:*".to_string()]);
        assert!(ctx.require_scope("catalog:read").is_ok());
        assert!(ctx.require_scope("admin:users").is_ok());
    }

    #[test]
    fn require_scope_oauth_empty_scopes_rejects() {
        let ctx = make_auth_ctx(Some(Uuid::new_v4()), vec![]);
        assert!(ctx.require_scope("catalog:read").is_err());
    }

    #[test]
    fn require_scope_error_message_includes_scope() {
        let ctx = make_auth_ctx(Some(Uuid::new_v4()), vec!["catalog:read".to_string()]);
        let err = ctx.require_scope("admin:users").unwrap_err();
        let msg = err.message.to_string();
        assert!(msg.contains("admin:users"));
        assert!(msg.contains("catalog:read"));
    }

    #[test]
    fn scope_matches_exact_and_wildcard_forms() {
        let allowed = vec!["catalog:*".to_string(), "orders:read".to_string()];
        assert!(scope_matches(&allowed, "catalog:read"));
        assert!(scope_matches(&allowed, "catalog:write"));
        assert!(scope_matches(&allowed, "orders:read"));
        assert!(!scope_matches(&allowed, "orders:write"));
    }

    #[test]
    fn effective_permission_accepts_manage_permission() {
        let permissions = vec![Permission::PAGES_MANAGE];
        assert!(has_effective_permission(
            &permissions,
            &Permission::PAGES_UPDATE,
        ));
        assert!(has_any_effective_permission(
            &permissions,
            &[Permission::PAGES_CREATE, Permission::PAGES_DELETE],
        ));
    }
}
