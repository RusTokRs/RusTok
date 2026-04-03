use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacAdminBootstrap {
    pub tenant_slug: String,
    pub current_user_id: String,
    pub inferred_role: String,
    pub granted_permissions: Vec<String>,
    pub module_permissions: Vec<RbacModulePermissionGroup>,
    pub host_surfaces: Vec<RbacHostSurfaceLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacModulePermissionGroup {
    pub module_slug: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacHostSurfaceLink {
    pub label: String,
    pub href: String,
}
