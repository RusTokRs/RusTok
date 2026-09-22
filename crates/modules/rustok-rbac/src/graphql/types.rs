use async_graphql::{Enum, SimpleObject};
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
    /// Role slug, e.g. "super_admin", "admin", "manager", "customer"
    pub slug: String,
    /// Human-readable display name
    pub display_name: String,
    /// All permissions granted to this role (e.g. "users:create")
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AssignUserRolePayload {
    pub success: bool,
    pub user_id: String,
    pub role: String,
}
