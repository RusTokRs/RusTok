use async_graphql::{Enum, InputObject, SimpleObject};
use rustok_core::UserRole;

#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum RbacGraphqlUserRole {
    SuperAdmin,
    Admin,
    Manager,
    Customer,
}

impl From<RbacGraphqlUserRole> for UserRole {
    fn from(role: RbacGraphqlUserRole) -> Self {
        match role {
            RbacGraphqlUserRole::SuperAdmin => UserRole::SuperAdmin,
            RbacGraphqlUserRole::Admin => UserRole::Admin,
            RbacGraphqlUserRole::Manager => UserRole::Manager,
            RbacGraphqlUserRole::Customer => UserRole::Customer,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RoleInfo {
    /// Role ID if persisted in database
    pub id: Option<String>,
    /// Role slug, e.g. "super_admin", "admin", "manager", "customer"
    pub slug: String,
    /// Human-readable display name
    pub display_name: String,
    /// Role description
    pub description: Option<String>,
    /// Whether this is a platform-owned built-in role that cannot be deleted
    pub is_system: bool,
    /// All permissions granted to this role (e.g. "users:create")
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct PlatformPermissionItem {
    /// Canonical permission identifier, e.g. "users:create"
    pub id: String,
    /// Resource name, e.g. "users"
    pub resource: String,
    /// Action name, e.g. "create"
    pub action: String,
    /// Optional permission description
    pub description: Option<String>,
}

#[derive(InputObject)]
pub struct CreateRoleInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(InputObject)]
pub struct UpdateRoleInput {
    pub slug: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RoleMutationPayload {
    pub success: bool,
    pub role: Option<RoleInfo>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct DeleteRolePayload {
    pub success: bool,
    pub slug: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AssignUserRolePayload {
    pub success: bool,
    pub user_id: String,
    pub role: String,
}
