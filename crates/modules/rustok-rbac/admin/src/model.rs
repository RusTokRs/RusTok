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
    #[serde(default)]
    pub id: Option<String>,
    pub slug: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_system: bool,
    pub permissions: Vec<String>,
}
