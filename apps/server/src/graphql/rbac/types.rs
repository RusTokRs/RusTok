use async_graphql::SimpleObject;

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
