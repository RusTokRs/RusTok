use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexAdminBootstrap {
    pub tenant: IndexTenantSnapshot,
    pub module: IndexModuleSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexTenantSnapshot {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub default_locale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexModuleSnapshot {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub rewrite_status: String,
    pub current_milestone: String,
}
