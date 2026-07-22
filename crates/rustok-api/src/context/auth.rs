use crate::{Action, Permission, Resource};
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};
use std::collections::HashSet;
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

/// Apply the OAuth maximum-authority boundary to an RBAC permission snapshot.
///
/// Direct grants should not call this function. OAuth principals receive only
/// permissions admitted by both their RBAC/app permission snapshot and the
/// scopes embedded in the access token. `manage` permissions are expanded into
/// scoped concrete actions when a scope grants only read or write authority, so
/// a broad RBAC role cannot bypass a narrow OAuth scope.
pub fn restrict_permissions_to_scopes(
    permissions: &[Permission],
    scopes: &[String],
) -> Vec<Permission> {
    if scopes.iter().any(|scope| scope == "*:*") {
        return permissions.to_vec();
    }

    let mut seen = HashSet::new();
    let mut restricted = Vec::new();

    for permission in permissions {
        if permission.action == Action::Manage {
            if scopes_allow_permission(scopes, permission) && seen.insert(*permission) {
                restricted.push(*permission);
            }

            for action in SCOPABLE_ACTIONS {
                let concrete = Permission::new(permission.resource, action);
                if scopes_allow_permission(scopes, &concrete) && seen.insert(concrete) {
                    restricted.push(concrete);
                }
            }
        } else if scopes_allow_permission(scopes, permission) && seen.insert(*permission) {
            restricted.push(*permission);
        }
    }

    restricted
}

const SCOPABLE_ACTIONS: [Action; 14] = [
    Action::Create,
    Action::Read,
    Action::Update,
    Action::Delete,
    Action::List,
    Action::Export,
    Action::Import,
    Action::Publish,
    Action::Moderate,
    Action::Execute,
    Action::Run,
    Action::Cancel,
    Action::Resolve,
    Action::Override,
];

fn scopes_allow_permission(scopes: &[String], permission: &Permission) -> bool {
    scopes
        .iter()
        .any(|scope| scope_allows_permission(scope, permission))
}

fn scope_allows_permission(scope: &str, permission: &Permission) -> bool {
    if scope == "*:*" || scope == "admin:*" {
        return true;
    }

    match scope {
        "admin:users" => {
            return matches!(
                permission.resource,
                Resource::Users | Resource::Customers | Resource::Profiles
            );
        }
        "admin:tenants" => return permission.resource == Resource::Tenants,
        "admin:modules" => return permission.resource == Resource::Modules,
        "admin:settings" => return permission.resource == Resource::Settings,
        "admin:builds" => {
            return matches!(
                permission.resource,
                Resource::Workflows | Resource::WorkflowExecutions
            );
        }
        "storefront:*" => return is_storefront_resource(permission.resource),
        _ => {}
    }

    let Some((resource_selector, action_selector)) = scope.rsplit_once(':') else {
        return false;
    };

    resource_selector_matches(resource_selector, permission.resource)
        && action_selector_matches(action_selector, permission.action)
}

fn resource_selector_matches(selector: &str, resource: Resource) -> bool {
    let resource_name = resource.to_string();
    if selector == resource_name {
        return true;
    }

    match selector {
        "catalog" => matches!(
            resource,
            Resource::Products
                | Resource::Categories
                | Resource::Inventory
                | Resource::Discounts
                | Resource::Regions
        ),
        "content" => matches!(
            resource,
            Resource::FlexSchemas
                | Resource::FlexEntries
                | Resource::Posts
                | Resource::BlogPosts
                | Resource::Pages
                | Resource::Nodes
                | Resource::Media
                | Resource::Seo
                | Resource::Comments
                | Resource::Tags
                | Resource::Taxonomy
        ),
        "orders" => matches!(
            resource,
            Resource::Orders | Resource::Payments | Resource::Fulfillments
        ),
        "users" => matches!(
            resource,
            Resource::Users | Resource::Customers | Resource::Profiles
        ),
        "forum" => resource_name.starts_with("forum_"),
        "ai" => resource_name.starts_with("ai:"),
        _ => false,
    }
}

fn action_selector_matches(selector: &str, action: Action) -> bool {
    match selector {
        "*" => true,
        "read" => matches!(action, Action::Read | Action::List | Action::Export),
        "write" => matches!(
            action,
            Action::Create
                | Action::Update
                | Action::Delete
                | Action::Import
                | Action::Publish
                | Action::Moderate
                | Action::Execute
                | Action::Run
                | Action::Cancel
                | Action::Resolve
                | Action::Override
        ),
        value => value == action.to_string(),
    }
}

fn is_storefront_resource(resource: Resource) -> bool {
    matches!(
        resource,
        Resource::Products
            | Resource::Categories
            | Resource::Orders
            | Resource::Customers
            | Resource::Profiles
            | Resource::Regions
            | Resource::Payments
            | Resource::Fulfillments
            | Resource::Inventory
            | Resource::Discounts
            | Resource::Posts
            | Resource::BlogPosts
            | Resource::Pages
            | Resource::Nodes
            | Resource::Media
            | Resource::Seo
            | Resource::Comments
            | Resource::Tags
            | Resource::Taxonomy
            | Resource::ForumCategories
            | Resource::ForumTopics
            | Resource::ForumReplies
    )
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

    #[test]
    fn catalog_read_scope_does_not_leak_manage_write_authority() {
        let restricted = restrict_permissions_to_scopes(
            &[Permission::PRODUCTS_MANAGE],
            &["catalog:read".to_string()],
        );

        assert!(restricted.contains(&Permission::PRODUCTS_READ));
        assert!(restricted.contains(&Permission::PRODUCTS_LIST));
        assert!(!restricted.contains(&Permission::PRODUCTS_UPDATE));
        assert!(!restricted.contains(&Permission::PRODUCTS_MANAGE));
    }

    #[test]
    fn forum_namespace_scope_preserves_forum_manage_permission() {
        let permission = Permission::new(Resource::ForumTopics, Action::Manage);
        let restricted = restrict_permissions_to_scopes(&[permission], &["forum:*".to_string()]);
        assert_eq!(restricted, vec![permission]);
    }

    #[test]
    fn admin_users_scope_does_not_admit_settings_management() {
        let restricted = restrict_permissions_to_scopes(
            &[Permission::USERS_MANAGE, Permission::SETTINGS_MANAGE],
            &["admin:users".to_string()],
        );
        assert!(restricted.contains(&Permission::USERS_MANAGE));
        assert!(!restricted.contains(&Permission::SETTINGS_MANAGE));
    }

    #[test]
    fn empty_oauth_scopes_resolve_no_permissions() {
        assert!(restrict_permissions_to_scopes(&[Permission::USERS_READ], &[]).is_empty());
    }
}
