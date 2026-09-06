use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacAdminBootstrap {
    pub tenant_slug: String,
    pub current_user_id: String,
    pub inferred_role: String,
    pub granted_permissions: Vec<String>,
    pub module_permissions: Vec<RbacModulePermissionGroup>,
    pub roles: Vec<RbacRoleInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacModulePermissionGroup {
    pub module_slug: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacRoleInfo {
    pub slug: String,
    pub display_name: String,
    pub permissions: Vec<String>,
}
